use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

use logcompact_core::{
    Diagnostic, DiagnosticClass, Emitter, EvidenceQuality, Location, LogLine, Parser, Provenance,
    ScopeBoundary, Severity, Stream,
};
use regex::{Captures, Regex, RegexBuilder};
use serde::Deserialize;
use serde_json::Value;

/// Hard limits for untrusted problem-matcher definitions and partial matches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProblemMatcherLimits {
    pub max_document_bytes: usize,
    pub max_matchers: usize,
    pub max_patterns_per_matcher: usize,
    pub max_total_regex_bytes: usize,
    pub max_compiled_regex_bytes: usize,
    pub max_active_states: usize,
    pub max_state_bytes: usize,
}

impl Default for ProblemMatcherLimits {
    fn default() -> Self {
        Self {
            max_document_bytes: 1024 * 1024,
            max_matchers: 64,
            max_patterns_per_matcher: 8,
            max_total_regex_bytes: 64 * 1024,
            max_compiled_regex_bytes: 2 * 1024 * 1024,
            max_active_states: 512,
            max_state_bytes: 256 * 1024,
        }
    }
}

/// Invalid or unsupported problem-matcher configuration.
#[derive(Debug)]
pub enum ProblemMatcherError {
    Json(serde_json::Error),
    Invalid(String),
}

impl Display for ProblemMatcherError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "invalid problem matcher JSON: {error}"),
            Self::Invalid(message) => formatter.write_str(message),
        }
    }
}

impl Error for ProblemMatcherError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}

impl From<serde_json::Error> for ProblemMatcherError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

/// Ordered registry compiled before parsing begins. A later owner replaces an
/// earlier owner, matching GitHub Actions registration behavior.
#[derive(Clone)]
pub struct ProblemMatcherRegistry {
    limits: ProblemMatcherLimits,
    matchers: Vec<CompiledMatcher>,
}

impl ProblemMatcherRegistry {
    #[must_use]
    pub fn new(limits: ProblemMatcherLimits) -> Self {
        Self {
            limits,
            matchers: Vec::new(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.matchers.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.matchers.len()
    }

    /// Returns the built-in matcher owners replaced by this registry.
    #[must_use]
    pub fn builtin_overrides(&self) -> crate::BuiltinMatcherOverrides {
        crate::BuiltinMatcherOverrides::from_owners(
            self.matchers.iter().map(|matcher| matcher.owner.as_str()),
        )
    }

    /// Compiles one GitHub problem-matcher document, a VS Code inline matcher,
    /// or an array of inline matchers. The registry is unchanged on failure.
    pub fn add_json(&mut self, input: &[u8]) -> Result<(), ProblemMatcherError> {
        if input.len() > self.limits.max_document_bytes {
            return Err(invalid(format!(
                "problem matcher document is {} bytes; maximum is {}",
                input.len(),
                self.limits.max_document_bytes
            )));
        }
        let value: Value = serde_json::from_slice(input)?;
        let raw = raw_matchers(value)?;
        if raw.len() > self.limits.max_matchers {
            return Err(invalid(format!(
                "problem matcher document contains {} entries; maximum is {}",
                raw.len(),
                self.limits.max_matchers
            )));
        }
        let compiled = raw
            .into_iter()
            .enumerate()
            .map(|(index, matcher)| compile_matcher(matcher, index, self.limits))
            .collect::<Result<Vec<_>, _>>()?;

        let mut next = self.matchers.clone();
        for matcher in compiled {
            next.retain(|existing| existing.owner != matcher.owner);
            next.push(matcher);
        }
        if next.len() > self.limits.max_matchers {
            return Err(invalid(format!(
                "problem matcher registry contains {} owners; maximum is {}",
                next.len(),
                self.limits.max_matchers
            )));
        }
        let regex_bytes = next
            .iter()
            .flat_map(|matcher| &matcher.patterns)
            .map(|pattern| pattern.source_bytes)
            .fold(0_usize, usize::saturating_add);
        if regex_bytes > self.limits.max_total_regex_bytes {
            return Err(invalid(format!(
                "problem matcher regex sources total {regex_bytes} bytes; maximum is {}",
                self.limits.max_total_regex_bytes
            )));
        }
        self.matchers = next;
        Ok(())
    }

    #[must_use]
    pub fn into_parser(self) -> ProblemMatcherParser {
        ProblemMatcherParser {
            matchers: self.matchers,
            limits: self.limits,
            states: BTreeMap::new(),
        }
    }
}

impl Default for ProblemMatcherRegistry {
    fn default() -> Self {
        Self::new(ProblemMatcherLimits::default())
    }
}

/// Bounded, deterministic single-line and consecutive multiline matcher.
pub struct ProblemMatcherParser {
    matchers: Vec<CompiledMatcher>,
    limits: ProblemMatcherLimits,
    states: BTreeMap<(String, Stream, usize), MatchState>,
}

impl Parser for ProblemMatcherParser {
    fn id(&self) -> &'static str {
        "problem-matcher.v1"
    }

    fn observe(&mut self, line: &LogLine<'_>, emitter: &mut Emitter<'_>) {
        let limits = self.limits;
        for matcher_index in 0..self.matchers.len() {
            observe_matcher(
                &self.matchers[matcher_index],
                matcher_index,
                line,
                emitter,
                &mut self.states,
                limits,
                "problem-matcher.v1",
            );
        }
    }

    fn boundary(&mut self, boundary: ScopeBoundary<'_>, _emitter: &mut Emitter<'_>) {
        if let ScopeBoundary::End { scope, .. } = boundary {
            self.states
                .retain(|(scope_id, _, _), _| scope_id != &scope.id);
        }
    }

    fn finish(&mut self, _emitter: &mut Emitter<'_>) {
        self.states.clear();
    }
}

#[derive(Clone)]
struct CompiledMatcher {
    owner: String,
    source: Option<String>,
    severity: Severity,
    file_location: FileLocation,
    patterns: Vec<CompiledPattern>,
}

#[derive(Clone)]
struct CompiledPattern {
    regex: Regex,
    source_bytes: usize,
    kind: Option<LocationKind>,
    file: Option<usize>,
    from_path: Option<usize>,
    location: Option<usize>,
    line: Option<usize>,
    column: Option<usize>,
    end_line: Option<usize>,
    end_column: Option<usize>,
    severity: Option<usize>,
    code: Option<usize>,
    message: Option<usize>,
    r#loop: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LocationKind {
    File,
    Location,
}

#[derive(Clone)]
enum FileLocation {
    Captured,
    Relative(String),
}

#[derive(Clone, Default)]
struct CapturedProblem {
    file: Option<String>,
    from_path: Option<String>,
    location: Option<String>,
    line: Option<String>,
    column: Option<String>,
    end_line: Option<String>,
    end_column: Option<String>,
    severity: Option<String>,
    code: Option<String>,
    message: Option<String>,
}

impl CapturedProblem {
    fn bytes(&self) -> usize {
        [
            &self.file,
            &self.from_path,
            &self.location,
            &self.line,
            &self.column,
            &self.end_line,
            &self.end_column,
            &self.severity,
            &self.code,
            &self.message,
        ]
        .into_iter()
        .filter_map(|value| value.as_ref())
        .map(String::len)
        .fold(0_usize, usize::saturating_add)
    }
}

struct MatchState {
    next_pattern: usize,
    data: CapturedProblem,
    start_line: u64,
}

fn observe_matcher(
    matcher: &CompiledMatcher,
    matcher_index: usize,
    line: &LogLine<'_>,
    emitter: &mut Emitter<'_>,
    states: &mut BTreeMap<(String, Stream, usize), MatchState>,
    limits: ProblemMatcherLimits,
    parser_id: &'static str,
) {
    let key = (line.scope.id.clone(), line.stream, matcher_index);
    if let Some(mut state) = states.remove(&key) {
        let pattern = &matcher.patterns[state.next_pattern];
        if let Some(captures) = pattern.regex.captures(line.text) {
            if state.next_pattern == matcher.patterns.len() - 1 {
                if pattern.r#loop {
                    let mut finding = state.data.clone();
                    apply_captures(&mut finding, pattern, &captures);
                    emit_problem(
                        matcher,
                        finding,
                        line,
                        state.start_line,
                        emitter,
                        limits,
                        parser_id,
                    );
                    states.insert(key, state);
                } else {
                    apply_captures(&mut state.data, pattern, &captures);
                    emit_problem(
                        matcher,
                        state.data,
                        line,
                        state.start_line,
                        emitter,
                        limits,
                        parser_id,
                    );
                }
            } else {
                apply_captures(&mut state.data, pattern, &captures);
                state.next_pattern += 1;
                if retained_state_bytes(states).saturating_add(state.data.bytes())
                    <= limits.max_state_bytes
                {
                    states.insert(key, state);
                } else {
                    emitter.candidate_dropped();
                }
            }
            return;
        }
    }
    start_match(
        matcher,
        matcher_index,
        line,
        emitter,
        states,
        limits,
        parser_id,
    );
}

fn start_match(
    matcher: &CompiledMatcher,
    matcher_index: usize,
    line: &LogLine<'_>,
    emitter: &mut Emitter<'_>,
    states: &mut BTreeMap<(String, Stream, usize), MatchState>,
    limits: ProblemMatcherLimits,
    parser_id: &'static str,
) {
    let pattern = &matcher.patterns[0];
    let Some(captures) = pattern.regex.captures(line.text) else {
        return;
    };
    let mut data = CapturedProblem::default();
    apply_captures(&mut data, pattern, &captures);
    if matcher.patterns.len() == 1 {
        emit_problem(
            matcher,
            data,
            line,
            line.stream_line,
            emitter,
            limits,
            parser_id,
        );
    } else if retained_state_bytes(states).saturating_add(data.bytes()) > limits.max_state_bytes
        || states.len() >= limits.max_active_states
    {
        emitter.candidate_dropped();
    } else {
        states.insert(
            (line.scope.id.clone(), line.stream, matcher_index),
            MatchState {
                next_pattern: 1,
                data,
                start_line: line.stream_line,
            },
        );
    }
}

fn retained_state_bytes(states: &BTreeMap<(String, Stream, usize), MatchState>) -> usize {
    states
        .values()
        .map(|state| state.data.bytes())
        .fold(0_usize, usize::saturating_add)
}

fn apply_captures(data: &mut CapturedProblem, pattern: &CompiledPattern, captures: &Captures<'_>) {
    fill(&mut data.file, pattern.file, captures, true);
    fill(&mut data.from_path, pattern.from_path, captures, true);
    fill(&mut data.location, pattern.location, captures, true);
    fill(&mut data.line, pattern.line, captures, true);
    fill(&mut data.column, pattern.column, captures, true);
    fill(&mut data.end_line, pattern.end_line, captures, true);
    fill(&mut data.end_column, pattern.end_column, captures, true);
    fill(&mut data.severity, pattern.severity, captures, true);
    fill(&mut data.code, pattern.code, captures, true);
    append_message(&mut data.message, pattern.message, captures);
}

fn fill(target: &mut Option<String>, group: Option<usize>, captures: &Captures<'_>, trim: bool) {
    if target.is_some() {
        return;
    }
    let Some(value) = group.and_then(|index| captures.get(index)) else {
        return;
    };
    let value = if trim {
        value.as_str().trim()
    } else {
        value.as_str()
    };
    *target = Some(value.to_owned());
}

fn append_message(target: &mut Option<String>, group: Option<usize>, captures: &Captures<'_>) {
    let Some(value) = group.and_then(|index| captures.get(index)) else {
        return;
    };
    let value = value.as_str().trim();
    if let Some(existing) = target {
        existing.push('\n');
        existing.push_str(value);
    } else {
        *target = Some(value.to_owned());
    }
}

fn emit_problem(
    matcher: &CompiledMatcher,
    data: CapturedProblem,
    line: &LogLine<'_>,
    start_line: u64,
    emitter: &mut Emitter<'_>,
    limits: ProblemMatcherLimits,
    parser_id: &'static str,
) {
    if data.bytes() > limits.max_state_bytes {
        emitter.candidate_dropped();
        return;
    }
    let Some(file) = data.file.as_deref().filter(|value| !value.is_empty()) else {
        return;
    };
    let path = matcher
        .file_location
        .resolve(data.from_path.as_deref(), file);
    let location = location(
        &data,
        path,
        matcher.patterns[0].kind.unwrap_or(LocationKind::Location),
    );
    let Some(location) = location else {
        return;
    };
    let severity = data
        .severity
        .as_deref()
        .and_then(parse_severity)
        .unwrap_or(matcher.severity);
    let Some(message) = data.message.filter(|value| !value.is_empty()) else {
        return;
    };
    let label = matcher.source.as_deref().unwrap_or(&matcher.owner);
    emitter.diagnostic(Diagnostic {
        severity,
        class: DiagnosticClass::Tool,
        code: data.code.filter(|value| !value.is_empty()),
        message,
        location: Some(location),
        provenance: Some(
            Provenance::new(stream_name(line.stream))
                .with_label(label)
                .with_scope(line.scope)
                .with_span(line.stream, start_line, line.stream_line)
                .with_parser(parser_id),
        ),
        quality: EvidenceQuality::Located,
        repetition_count: 1,
    });
}

fn location(data: &CapturedProblem, path: String, kind: LocationKind) -> Option<Location> {
    if kind == LocationKind::File {
        return Some(Location {
            path,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        });
    }
    let (line, column, end_line, end_column) = if let Some(value) = &data.location {
        parse_location(value)?
    } else {
        let line = parse_coordinate(data.line.as_deref()?)?;
        let column = data.column.as_deref().and_then(parse_coordinate);
        let end_line = data.end_line.as_deref().and_then(parse_coordinate);
        let end_column = data.end_column.as_deref().and_then(parse_coordinate);
        (line, column, end_line, end_column)
    };
    let end_line = end_line.or_else(|| end_column.map(|_| line));
    Some(Location {
        path,
        line: Some(line),
        column,
        end_line: end_line.map(|value| value.max(line)),
        end_column,
    })
}

type ParsedLocation = (u32, Option<u32>, Option<u32>, Option<u32>);

fn parse_location(value: &str) -> Option<ParsedLocation> {
    let parts = value.split(',').map(str::trim).collect::<Vec<_>>();
    match parts.as_slice() {
        [line] => Some((parse_coordinate(line)?, None, None, None)),
        [line, column] => Some((
            parse_coordinate(line)?,
            Some(parse_coordinate(column)?),
            None,
            None,
        )),
        [line, column, end_line, end_column] => Some((
            parse_coordinate(line)?,
            Some(parse_coordinate(column)?),
            Some(parse_coordinate(end_line)?),
            Some(parse_coordinate(end_column)?),
        )),
        _ => None,
    }
}

fn parse_coordinate(value: &str) -> Option<u32> {
    value.parse::<u32>().ok().filter(|value| *value > 0)
}

impl FileLocation {
    fn resolve(&self, from_path: Option<&str>, file: &str) -> String {
        let path = from_path.map_or_else(|| file.to_owned(), |root| join_path(root, file));
        match self {
            Self::Captured => path,
            Self::Relative(prefix) => join_path(prefix, &path),
        }
    }
}

fn join_path(prefix: &str, path: &str) -> String {
    if prefix.is_empty() || is_absolute(path) {
        return path.to_owned();
    }
    let prefix = prefix.trim_end_matches(['/', '\\']);
    let path = path.trim_start_matches(['/', '\\']);
    if prefix.is_empty() {
        path.to_owned()
    } else if path.is_empty() {
        prefix.to_owned()
    } else {
        format!("{prefix}/{path}")
    }
}

fn is_absolute(path: &str) -> bool {
    path.starts_with(['/', '\\'])
        || path
            .as_bytes()
            .get(1)
            .is_some_and(|delimiter| *delimiter == b':')
}

fn stream_name(stream: Stream) -> &'static str {
    match stream {
        Stream::Stdout => "stdout",
        Stream::Stderr => "stderr",
        Stream::Combined => "combined",
        Stream::Annotation => "annotation",
        Stream::Structured => "structured",
    }
}

fn parse_severity(value: &str) -> Option<Severity> {
    match value.trim().to_ascii_lowercase().as_str() {
        "error" | "err" | "e" => Some(Severity::Error),
        "warning" | "warn" | "w" => Some(Severity::Warning),
        "info" | "information" | "note" | "notice" | "hint" | "i" => Some(Severity::Note),
        _ => None,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawDocument {
    problem_matcher: OneOrMany<RawMatcherEntry>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> OneOrMany<T> {
    fn into_vec(self) -> Vec<T> {
        match self {
            Self::One(value) => vec![value],
            Self::Many(values) => values,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawMatcherEntry {
    Definition(Box<RawMatcher>),
    Reference(String),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawMatcher {
    owner: Option<String>,
    source: Option<String>,
    severity: Option<String>,
    file_location: Option<Value>,
    pattern: RawPatternSet,
    base: Option<String>,
    name: Option<String>,
    label: Option<String>,
    apply_to: Option<String>,
    background: Option<Value>,
    watching: Option<Value>,
    watched_task_begins_reg_exp: Option<String>,
    watched_task_ends_reg_exp: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawPatternSet {
    Single(RawPattern),
    Multiple(Vec<RawPattern>),
    Reference(String),
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawPattern {
    regexp: String,
    kind: Option<String>,
    file: Option<usize>,
    from_path: Option<usize>,
    location: Option<usize>,
    line: Option<usize>,
    column: Option<usize>,
    end_line: Option<usize>,
    end_column: Option<usize>,
    severity: Option<usize>,
    code: Option<usize>,
    message: Option<usize>,
    #[serde(default)]
    r#loop: bool,
}

fn raw_matchers(value: Value) -> Result<Vec<RawMatcherEntry>, ProblemMatcherError> {
    if value
        .as_object()
        .is_some_and(|object| object.contains_key("problemMatcher"))
    {
        return Ok(serde_json::from_value::<RawDocument>(value)?
            .problem_matcher
            .into_vec());
    }
    if value.is_array() {
        return Ok(serde_json::from_value::<Vec<RawMatcherEntry>>(value)?);
    }
    Ok(vec![serde_json::from_value::<RawMatcherEntry>(value)?])
}

fn compile_matcher(
    entry: RawMatcherEntry,
    index: usize,
    limits: ProblemMatcherLimits,
) -> Result<CompiledMatcher, ProblemMatcherError> {
    let raw = match entry {
        RawMatcherEntry::Definition(raw) => *raw,
        RawMatcherEntry::Reference(reference) => {
            return Err(invalid(format!(
                "problem matcher {index}: named matcher reference {reference:?} is not self-contained"
            )));
        }
    };
    if let Some(base) = raw.base {
        return Err(invalid(format!(
            "problem matcher {index}: base reference {base:?} cannot be resolved from a standalone definition"
        )));
    }
    if raw
        .apply_to
        .as_deref()
        .is_some_and(|value| !value.eq_ignore_ascii_case("allDocuments"))
    {
        return Err(invalid(format!(
            "problem matcher {index}: applyTo requires editor document state and only allDocuments is portable"
        )));
    }
    let _lifecycle_metadata = (
        raw.name,
        raw.label,
        raw.background,
        raw.watching,
        raw.watched_task_begins_reg_exp,
        raw.watched_task_ends_reg_exp,
    );
    let owner = required_bounded(raw.owner, "owner", index, 1024)?;
    let source = bounded_optional(raw.source, "source", index, 1024)?;
    let severity = raw
        .severity
        .as_deref()
        .map(|value| {
            parse_severity(value).ok_or_else(|| {
                invalid(format!(
                    "problem matcher {index} ({owner:?}): invalid default severity {value:?}"
                ))
            })
        })
        .transpose()?
        .unwrap_or(Severity::Error);
    let file_location = compile_file_location(raw.file_location, index, &owner)?;
    let mut patterns = match raw.pattern {
        RawPatternSet::Single(pattern) => vec![pattern],
        RawPatternSet::Multiple(patterns) => patterns,
        RawPatternSet::Reference(reference) => {
            return Err(invalid(format!(
                "problem matcher {index} ({owner:?}): named pattern reference {reference:?} cannot be resolved"
            )));
        }
    };
    if patterns.is_empty() {
        return Err(invalid(format!(
            "problem matcher {index} ({owner:?}): pattern must not be empty"
        )));
    }
    if patterns.len() > limits.max_patterns_per_matcher {
        return Err(invalid(format!(
            "problem matcher {index} ({owner:?}) has {} patterns; maximum is {}",
            patterns.len(),
            limits.max_patterns_per_matcher
        )));
    }
    apply_single_line_defaults(&mut patterns);
    validate_pattern_contract(&patterns, index, &owner)?;
    let patterns = patterns
        .into_iter()
        .enumerate()
        .map(|(pattern_index, pattern)| {
            compile_pattern(pattern, index, &owner, pattern_index, limits)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CompiledMatcher {
        owner,
        source,
        severity,
        file_location,
        patterns,
    })
}

fn apply_single_line_defaults(patterns: &mut [RawPattern]) {
    if patterns.len() != 1 {
        return;
    }
    let pattern = &mut patterns[0];
    pattern.file.get_or_insert(1);
    if pattern.location.is_none()
        && !pattern
            .kind
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("file"))
    {
        pattern.line.get_or_insert(2);
        pattern.column.get_or_insert(3);
    }
    pattern.message.get_or_insert(0);
}

fn validate_pattern_contract(
    patterns: &[RawPattern],
    matcher_index: usize,
    owner: &str,
) -> Result<(), ProblemMatcherError> {
    let mut has_file = false;
    let mut has_message = false;
    let mut has_location = false;
    for (index, pattern) in patterns.iter().enumerate() {
        if index > 0 && pattern.kind.is_some() {
            return Err(invalid(format!(
                "problem matcher {matcher_index} ({owner:?}) pattern {index}: kind is only valid on the first pattern"
            )));
        }
        if pattern.r#loop && (index != patterns.len() - 1 || patterns.len() == 1) {
            return Err(invalid(format!(
                "problem matcher {matcher_index} ({owner:?}) pattern {index}: loop is only valid on the last pattern of a multiline matcher"
            )));
        }
        has_file |= pattern.file.is_some();
        has_message |= pattern.message.is_some();
        has_location |= pattern.location.is_some() || pattern.line.is_some();
    }
    if !has_file || !has_message {
        return Err(invalid(format!(
            "problem matcher {matcher_index} ({owner:?}) must capture a file and message"
        )));
    }
    let kind = patterns[0]
        .kind
        .as_deref()
        .map(parse_location_kind)
        .transpose()?
        .unwrap_or(LocationKind::Location);
    if kind == LocationKind::Location && !has_location {
        return Err(invalid(format!(
            "problem matcher {matcher_index} ({owner:?}) must capture line or location unless kind is file"
        )));
    }
    Ok(())
}

fn compile_pattern(
    raw: RawPattern,
    matcher_index: usize,
    owner: &str,
    pattern_index: usize,
    limits: ProblemMatcherLimits,
) -> Result<CompiledPattern, ProblemMatcherError> {
    let regex = RegexBuilder::new(&raw.regexp)
        .size_limit(limits.max_compiled_regex_bytes)
        .dfa_size_limit(limits.max_compiled_regex_bytes)
        .build()
        .map_err(|error| {
            invalid(format!(
                "problem matcher {matcher_index} ({owner:?}) pattern {pattern_index}: invalid or unsupported regular expression: {error}"
            ))
        })?;
    let pattern = CompiledPattern {
        regex,
        source_bytes: raw.regexp.len(),
        kind: raw.kind.as_deref().map(parse_location_kind).transpose()?,
        file: raw.file,
        from_path: raw.from_path,
        location: raw.location,
        line: raw.line,
        column: raw.column,
        end_line: raw.end_line,
        end_column: raw.end_column,
        severity: raw.severity,
        code: raw.code,
        message: raw.message,
        r#loop: raw.r#loop,
    };
    let capture_count = pattern.regex.captures_len();
    for (field, group) in pattern.capture_groups() {
        if group >= capture_count {
            return Err(invalid(format!(
                "problem matcher {matcher_index} ({owner:?}) pattern {pattern_index}: {field} references capture group {group}, but the regexp has {} groups",
                capture_count.saturating_sub(1)
            )));
        }
    }
    Ok(pattern)
}

impl CompiledPattern {
    fn capture_groups(&self) -> Vec<(&'static str, usize)> {
        [
            ("file", self.file),
            ("fromPath", self.from_path),
            ("location", self.location),
            ("line", self.line),
            ("column", self.column),
            ("endLine", self.end_line),
            ("endColumn", self.end_column),
            ("severity", self.severity),
            ("code", self.code),
            ("message", self.message),
        ]
        .into_iter()
        .filter_map(|(field, value)| value.map(|group| (field, group)))
        .collect()
    }
}

fn compile_file_location(
    value: Option<Value>,
    matcher_index: usize,
    owner: &str,
) -> Result<FileLocation, ProblemMatcherError> {
    let Some(value) = value else {
        return Ok(FileLocation::Captured);
    };
    if let Some(kind) = value.as_str() {
        return match kind.to_ascii_lowercase().as_str() {
            "relative" | "absolute" | "autodetect" => Ok(FileLocation::Captured),
            "search" => Err(invalid(format!(
                "problem matcher {matcher_index} ({owner:?}): fileLocation search requires filesystem access"
            ))),
            _ => Err(invalid(format!(
                "problem matcher {matcher_index} ({owner:?}): invalid fileLocation {kind:?}"
            ))),
        };
    }
    let Some(values) = value.as_array() else {
        return Err(invalid(format!(
            "problem matcher {matcher_index} ({owner:?}): fileLocation must be a string or pair"
        )));
    };
    if values.len() != 2 {
        return Err(invalid(format!(
            "problem matcher {matcher_index} ({owner:?}): fileLocation pair must have two elements"
        )));
    }
    let Some(kind) = values[0].as_str() else {
        return Err(invalid(format!(
            "problem matcher {matcher_index} ({owner:?}): fileLocation kind must be a string"
        )));
    };
    if kind.eq_ignore_ascii_case("search") {
        return Err(invalid(format!(
            "problem matcher {matcher_index} ({owner:?}): fileLocation search requires filesystem access"
        )));
    }
    if !kind.eq_ignore_ascii_case("relative") && !kind.eq_ignore_ascii_case("autodetect") {
        return Err(invalid(format!(
            "problem matcher {matcher_index} ({owner:?}): fileLocation pair must use relative or autodetect"
        )));
    }
    let Some(prefix) = values[1].as_str() else {
        return Err(invalid(format!(
            "problem matcher {matcher_index} ({owner:?}): fileLocation prefix must be a string"
        )));
    };
    if prefix.len() > 4096 {
        return Err(invalid(format!(
            "problem matcher {matcher_index} ({owner:?}): fileLocation prefix exceeds 4096 bytes"
        )));
    }
    Ok(FileLocation::Relative(prefix.to_owned()))
}

fn parse_location_kind(value: &str) -> Result<LocationKind, ProblemMatcherError> {
    match value.to_ascii_lowercase().as_str() {
        "file" => Ok(LocationKind::File),
        "location" => Ok(LocationKind::Location),
        _ => Err(invalid(format!("invalid problem matcher kind {value:?}"))),
    }
}

fn required_bounded(
    value: Option<String>,
    field: &str,
    matcher_index: usize,
    maximum: usize,
) -> Result<String, ProblemMatcherError> {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return Err(invalid(format!(
            "problem matcher {matcher_index}: {field} is required and must not be empty"
        )));
    };
    if value.len() > maximum {
        return Err(invalid(format!(
            "problem matcher {matcher_index}: {field} exceeds {maximum} bytes"
        )));
    }
    Ok(value)
}

fn bounded_optional(
    value: Option<String>,
    field: &str,
    matcher_index: usize,
    maximum: usize,
) -> Result<Option<String>, ProblemMatcherError> {
    if value.as_ref().is_some_and(|value| value.len() > maximum) {
        return Err(invalid(format!(
            "problem matcher {matcher_index}: {field} exceeds {maximum} bytes"
        )));
    }
    Ok(value)
}

fn invalid(message: String) -> ProblemMatcherError {
    ProblemMatcherError::Invalid(message)
}

#[cfg(test)]
mod tests {
    use logcompact_core::{
        Budget, EndReason, GenericRanker, NoPathMapping, NoRedaction, OutputPolicy, ParserPlan,
        Reduction, ReductionSession, Scope, SessionOptions,
    };

    use super::*;

    fn reduce(definition: &[u8], chunks: &[&[u8]]) -> Reduction {
        let mut registry = ProblemMatcherRegistry::default();
        registry.add_json(definition).unwrap();
        let mut plan = ParserPlan::new();
        plan.push(registry.into_parser()).unwrap();
        let mut session = ReductionSession::new(
            plan,
            SessionOptions {
                budget: Budget::unbounded(),
                ..SessionOptions::default()
            },
            OutputPolicy::new(&NoRedaction, &NoPathMapping, &GenericRanker),
        );
        session.begin_scope(Scope::step("matcher"));
        for chunk in chunks {
            session.push_chunk("matcher", Stream::Stderr, chunk);
        }
        session.end_scope("matcher", EndReason::Complete);
        session.finish()
    }

    fn reduce_with_builtins(definition: &[u8], input: &[u8]) -> Reduction {
        let mut registry = ProblemMatcherRegistry::default();
        registry.add_json(definition).unwrap();
        let plan = crate::builtin_parser_plan_with_problem_matchers(
            crate::BuiltinParserOptions::default(),
            registry,
        );
        let mut session = ReductionSession::new(
            plan,
            SessionOptions {
                budget: Budget::unbounded(),
                ..SessionOptions::default()
            },
            OutputPolicy::new(&NoRedaction, &NoPathMapping, &GenericRanker),
        );
        session.begin_scope(Scope::step("matcher"));
        session.push_chunk("matcher", Stream::Stderr, input);
        session.end_scope("matcher", EndReason::Complete);
        session.finish()
    }

    #[test]
    fn parses_single_line_github_definition_with_ranges() {
        let definition = br#"{
            "problemMatcher": [{
                "owner": "example",
                "source": "example compiler",
                "fileLocation": ["relative", "workspace"],
                "severity": "warning",
                "pattern": [{
                    "regexp": "^([^|]+)\\|(.+):(\\d+):(\\d+)-(\\d+):(\\d+): (error|warning): (.+) \\[(.+)\\]$",
                    "fromPath": 1,
                    "file": 2,
                    "line": 3,
                    "column": 4,
                    "endLine": 5,
                    "endColumn": 6,
                    "severity": 7,
                    "message": 8,
                    "code": 9
                }]
            }]
        }"#;
        let reduction = reduce(
            definition,
            &[b"package|src/main.rs:12:4-13:8: error: unknown variable [E100]\n"],
        );
        assert_eq!(reduction.diagnostics.len(), 1);
        let diagnostic = &reduction.diagnostics[0];
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.code.as_deref(), Some("E100"));
        assert_eq!(diagnostic.message, "unknown variable");
        let location = diagnostic.location.as_ref().unwrap();
        assert_eq!(location.path, "workspace/package/src/main.rs");
        assert_eq!(location.line, Some(12));
        assert_eq!(location.column, Some(4));
        assert_eq!(location.end_line, Some(13));
        assert_eq!(location.end_column, Some(8));
        assert_eq!(
            diagnostic.provenance.as_ref().unwrap().label.as_deref(),
            Some("example compiler")
        );
    }

    #[test]
    fn multiline_loop_inherits_file_and_resets_on_noise() {
        let definition = br#"{
            "problemMatcher": [{
                "owner": "eslint-stylish",
                "pattern": [
                    {"regexp": "^([^\\s].*)$", "file": 1},
                    {
                        "regexp": "^\\s+(\\d+):(\\d+)\\s+(error|warning)\\s+(.+?)\\s{2,}(\\S+)$",
                        "line": 1,
                        "column": 2,
                        "severity": 3,
                        "message": 4,
                        "code": 5,
                        "loop": true
                    }
                ]
            }]
        }"#;
        let input = b"src/example.js\n  1:2 error Missing strict  strict\n  5:10 warning Unused value  unused\nnoise\nsrc/other.js\n  ignored\n  9:1 error Not consecutive  rule\n";
        let whole = reduce(definition, &[input]);
        let chunked = reduce(definition, &[&input[..17], &input[17..51], &input[51..]]);
        assert_eq!(whole, chunked);
        assert_eq!(whole.diagnostics.len(), 2);
        assert_eq!(
            whole.diagnostics[0].location.as_ref().unwrap().line,
            Some(1)
        );
        assert_eq!(
            whole.diagnostics[1].location.as_ref().unwrap().line,
            Some(5)
        );
        assert_eq!(whole.diagnostics[1].severity, Severity::Warning);
    }

    #[test]
    fn duplicate_owner_uses_last_registered_definition() {
        let first = br#"{"problemMatcher":[{"owner":"same","pattern":{"regexp":"^(.+):(\\d+):(\\d+): (.+)$","file":1,"line":2,"column":3,"message":4}}]}"#;
        let second = br#"{"problemMatcher":[{"owner":"same","severity":"warning","pattern":{"regexp":"^(.+):(\\d+):(\\d+): (.+)$","file":1,"line":2,"column":3,"message":4}}]}"#;
        let mut registry = ProblemMatcherRegistry::default();
        registry.add_json(first).unwrap();
        registry.add_json(second).unwrap();
        assert_eq!(registry.len(), 1);

        let mut plan = ParserPlan::new();
        plan.push(registry.into_parser()).unwrap();
        let mut session = ReductionSession::new(
            plan,
            SessionOptions {
                budget: Budget::unbounded(),
                ..SessionOptions::default()
            },
            OutputPolicy::new(&NoRedaction, &NoPathMapping, &GenericRanker),
        );
        session.begin_scope(Scope::step("matcher"));
        session.push_chunk("matcher", Stream::Stderr, b"file.rs:1:2: message\n");
        session.end_scope("matcher", EndReason::Complete);
        assert_eq!(session.finish().diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn custom_matchers_suppress_fallbacks_but_not_recognized_builtins() {
        let fallback_definition = br#"{
            "problemMatcher": [{
                "owner": "custom",
                "pattern": {
                    "regexp": "^CUSTOM (.+):(\\d+):(\\d+): error: (.+)$",
                    "file": 1, "line": 2, "column": 3, "message": 4
                }
            }]
        }"#;
        let fallback = reduce_with_builtins(
            fallback_definition,
            b"CUSTOM src/main.dsl:4:2: error: broken widget\n",
        );
        assert_eq!(fallback.diagnostics.len(), 1);
        assert_eq!(
            fallback.diagnostics[0]
                .provenance
                .as_ref()
                .unwrap()
                .parser
                .as_deref(),
            Some("problem-matcher.v1")
        );

        let go_definition = br#"{
            "problemMatcher": [{
                "owner": "custom-go",
                "pattern": {
                    "regexp": "^(.+):(\\d+):(\\d+): (.+)$",
                    "file": 1, "line": 2, "column": 3, "message": 4
                }
            }]
        }"#;
        let recognized =
            reduce_with_builtins(go_definition, b"src/main.go:12:4: undefined: total\n");
        assert_eq!(recognized.diagnostics.len(), 2);
        assert_eq!(recognized.diagnostics[0].class, DiagnosticClass::Compiler);
        assert_eq!(recognized.diagnostics[1].class, DiagnosticClass::Tool);
    }

    #[test]
    fn a_custom_owner_replaces_the_matching_builtin() {
        let definition = br#"{
            "problemMatcher": [{
                "owner": "rust-compiler",
                "source": "custom rust",
                "pattern": [
                    {
                        "regexp": "^error\\[(E\\d+)\\]: (.+)$",
                        "code": 1, "message": 2
                    },
                    {
                        "regexp": "^ --> (.+):(\\d+):(\\d+)$",
                        "file": 1, "line": 2, "column": 3
                    }
                ]
            }]
        }"#;
        let reduction = reduce_with_builtins(
            definition,
            b"error[E0308]: mismatched types\n --> src/lib.rs:7:5\n",
        );
        assert_eq!(reduction.diagnostics.len(), 1);
        assert_eq!(reduction.diagnostics[0].class, DiagnosticClass::Tool);
        assert_eq!(reduction.diagnostics[0].code.as_deref(), Some("E0308"));
        assert_eq!(reduction.diagnostics[0].message, "mismatched types");
        assert_eq!(
            reduction.diagnostics[0]
                .provenance
                .as_ref()
                .unwrap()
                .label
                .as_deref(),
            Some("custom rust")
        );
    }

    #[test]
    fn parses_combined_locations_and_file_only_findings() {
        let definition = br#"[
            {
                "owner": "range",
                "pattern": {
                    "regexp": "^RANGE (.+)\\((\\d+,\\d+,\\d+,\\d+)\\): (.+)$",
                    "file": 1,
                    "location": 2,
                    "message": 3
                }
            },
            {
                "owner": "whole-file",
                "pattern": {
                    "regexp": "^FILE (.+): (.+)$",
                    "kind": "file",
                    "file": 1,
                    "message": 2
                }
            }
        ]"#;
        let reduction = reduce(
            definition,
            &[b"RANGE src/lib.rs(2,3,4,5): ranged\nFILE src/all.rs: whole file\n"],
        );
        assert_eq!(reduction.diagnostics.len(), 2);
        let range = reduction.diagnostics[0].location.as_ref().unwrap();
        assert_eq!(range.line, Some(2));
        assert_eq!(range.column, Some(3));
        assert_eq!(range.end_line, Some(4));
        assert_eq!(range.end_column, Some(5));
        let file = reduction.diagnostics[1].location.as_ref().unwrap();
        assert_eq!(file.path, "src/all.rs");
        assert_eq!(file.line, None);
    }

    #[test]
    fn state_bounds_are_reported_as_truncation() {
        let definition = br#"{
            "problemMatcher": [{
                "owner": "bounded",
                "pattern": [
                    {"regexp": "^FILE (.+)$", "file": 1},
                    {"regexp": "^LINE (\\d+): (.+)$", "line": 1, "message": 2}
                ]
            }]
        }"#;
        let mut registry = ProblemMatcherRegistry::new(ProblemMatcherLimits {
            max_state_bytes: 4,
            ..ProblemMatcherLimits::default()
        });
        registry.add_json(definition).unwrap();
        let mut plan = ParserPlan::new();
        plan.push(registry.into_parser()).unwrap();
        let mut session = ReductionSession::new(
            plan,
            SessionOptions {
                budget: Budget::unbounded(),
                ..SessionOptions::default()
            },
            OutputPolicy::new(&NoRedaction, &NoPathMapping, &GenericRanker),
        );
        session.begin_scope(Scope::step("matcher"));
        session.push_chunk(
            "matcher",
            Stream::Stderr,
            b"FILE long-name.rs\nLINE 1: message\n",
        );
        session.end_scope("matcher", EndReason::Complete);
        let reduction = session.finish();
        assert!(reduction.diagnostics.is_empty());
        assert!(reduction.truncated);
        assert_eq!(reduction.stats.candidates_dropped, 1);
    }

    #[test]
    fn retained_state_bytes_are_bounded_across_scopes() {
        let definition = br#"{
            "problemMatcher": [{
                "owner": "bounded",
                "pattern": [
                    {"regexp": "^FILE (.+)$", "file": 1},
                    {"regexp": "^LINE (\\d+): (.+)$", "line": 1, "message": 2}
                ]
            }]
        }"#;
        let mut registry = ProblemMatcherRegistry::new(ProblemMatcherLimits {
            max_state_bytes: 7,
            ..ProblemMatcherLimits::default()
        });
        registry.add_json(definition).unwrap();
        let mut plan = ParserPlan::new();
        plan.push(registry.into_parser()).unwrap();
        let mut session = ReductionSession::new(
            plan,
            SessionOptions {
                budget: Budget::unbounded(),
                ..SessionOptions::default()
            },
            OutputPolicy::new(&NoRedaction, &NoPathMapping, &GenericRanker),
        );
        session.begin_scope(Scope::step("one"));
        session.begin_scope(Scope::step("two"));
        session.push_chunk("one", Stream::Stderr, b"FILE a.rs\n");
        session.push_chunk("two", Stream::Stderr, b"FILE b.rs\n");
        session.end_scope("one", EndReason::Complete);
        session.end_scope("two", EndReason::Complete);
        let reduction = session.finish();
        assert!(reduction.diagnostics.is_empty());
        assert!(reduction.truncated);
        assert_eq!(reduction.stats.candidates_dropped, 1);
    }

    #[test]
    fn rejects_unbounded_or_nonportable_definitions() {
        let mut registry = ProblemMatcherRegistry::default();
        for (definition, expected) in [
            (
                br#"{"problemMatcher":[{"owner":"bad","pattern":"$gcc"}]}"#.as_slice(),
                "named pattern reference",
            ),
            (
                br#"{"problemMatcher":[{"owner":"bad","fileLocation":"search","pattern":{"regexp":"^(.+):(\\d+):(.*)$","file":1,"line":2,"message":3}}]}"#.as_slice(),
                "filesystem access",
            ),
            (
                br#"{"problemMatcher":[{"owner":"bad","pattern":{"regexp":"^(a)$","file":1,"line":2,"message":3}}]}"#.as_slice(),
                "capture group 2",
            ),
            (
                br#"{"problemMatcher":[{"owner":"bad","pattern":{"regexp":"^(a)\\1$","file":1,"kind":"file","message":0}}]}"#.as_slice(),
                "unsupported regular expression",
            ),
        ] {
            let error = registry.add_json(definition).unwrap_err().to_string();
            assert!(error.contains(expected), "{error:?} did not contain {expected:?}");
            assert!(registry.is_empty());
        }
    }
}
