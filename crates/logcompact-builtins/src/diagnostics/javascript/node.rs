use std::collections::BTreeSet;

use crate::{Diagnostic, DiagnosticClass, Location, Severity};

use super::super::common::normalize_path;

pub(crate) fn reduce(
    input: &str,
    diagnostics: &mut Vec<Diagnostic>,
    messages: &mut BTreeSet<String>,
) {
    let mut parser = JavaScriptTestDiagnosticParser::default();
    for line in input.lines() {
        if let Some(diagnostic) = parser.observe_line(line) {
            messages.insert(diagnostic.message.clone());
            diagnostics.push(diagnostic);
        }
    }
    if let Some(diagnostic) = parser.finish() {
        messages.insert(diagnostic.message.clone());
        diagnostics.push(diagnostic);
    }
}

/// Stateful extractor for Node.js exceptions and their application frames.
#[derive(Debug, Default)]
pub struct JavaScriptTestDiagnosticParser {
    leading_location: Option<Location>,
    pending: Option<Diagnostic>,
    frames_seen: usize,
}

impl JavaScriptTestDiagnosticParser {
    const MAX_STACK_FRAMES: usize = 64;

    /// Observes one normalized test-log line and emits an exception after a
    /// JavaScript source header or application stack frame confirms it.
    pub fn observe_line(&mut self, line: &str) -> Option<Diagnostic> {
        if !line.trim_start().starts_with("at ")
            && let Some(location) = parse_location(line.trim())
        {
            let previous = self.take_confirmed();
            self.leading_location = Some(location);
            return previous;
        }
        if let Some(message) = exception_message(line) {
            let leading_location = self.leading_location.take();
            let previous = self.take_confirmed();
            self.pending = Some(Diagnostic {
                severity: Severity::Error,
                class: DiagnosticClass::Test,
                code: None,
                provenance: None,
                message: message.to_owned(),
                location: leading_location,
                quality: crate::EvidenceQuality::Structured,
                repetition_count: 1,
            });
            self.frames_seen = 0;
            return previous;
        }
        self.pending.as_ref()?;
        if let Some(location) = parse_stack_frame(line) {
            self.frames_seen = self.frames_seen.saturating_add(1);
            if let Some(location) = location {
                let mut diagnostic = self.pending.take()?;
                diagnostic.location = Some(location);
                self.frames_seen = 0;
                return Some(diagnostic);
            }
            if self.frames_seen >= Self::MAX_STACK_FRAMES {
                return self.take_confirmed();
            }
            return None;
        }
        if line.trim().is_empty() {
            return None;
        }
        self.take_confirmed()
    }

    /// Emits a confirmed exception that reached end-of-file.
    pub fn finish(&mut self) -> Option<Diagnostic> {
        self.take_confirmed()
    }

    fn take_confirmed(&mut self) -> Option<Diagnostic> {
        let confirmed = self
            .pending
            .as_ref()
            .is_some_and(|diagnostic| diagnostic.location.is_some())
            || self.frames_seen > 0;
        self.frames_seen = 0;
        self.leading_location = None;
        if confirmed {
            self.pending.take()
        } else {
            self.pending = None;
            None
        }
    }
}

pub(crate) fn exception_message(line: &str) -> Option<&str> {
    let line = line.trim();
    let exception_type = line.split_once(':').map_or(line, |(name, _)| name);
    let class_name = exception_type.split_whitespace().next()?;
    (!class_name.is_empty()
        && class_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$'))
        && (class_name.ends_with("Error") || class_name.ends_with("Exception")))
    .then_some(line)
}

fn parse_stack_frame(line: &str) -> Option<Option<Location>> {
    let frame = line.trim().strip_prefix("at ")?;
    let source = if let Some((_, source)) = frame.rsplit_once('(') {
        source.strip_suffix(')')?
    } else {
        frame.split_whitespace().last()?
    };
    if source.starts_with("node:") || matches!(source, "native" | "<anonymous>") {
        return Some(None);
    }
    let location = parse_location(source)?;
    let framework = location.path.contains("/node_modules/")
        || location.path.starts_with("node_modules/")
        || location.path.contains("/external/");
    Some((!framework).then_some(location))
}

fn parse_location(value: &str) -> Option<Location> {
    let value = value.trim().trim_matches('"');
    let path_end = path_end(value)?;
    let coordinates = value[path_end..].strip_prefix(':')?;
    let (line_number, column) = if let Some((line_number, column)) = coordinates.split_once(':') {
        (
            line_number.parse::<u32>().ok()?,
            Some(column.parse::<u32>().ok()?),
        )
    } else {
        (coordinates.parse::<u32>().ok()?, None)
    };
    Some(Location {
        path: compact_path(&value[..path_end]),
        line: Some(line_number),
        column,
        end_line: None,
        end_column: None,
    })
}

fn path_end(value: &str) -> Option<usize> {
    const EXTENSIONS: [&str; 8] = [".tsx", ".mts", ".cts", ".ts", ".jsx", ".mjs", ".cjs", ".js"];
    EXTENSIONS
        .iter()
        .filter_map(|extension| {
            value.rmatch_indices(extension).find_map(|(index, _)| {
                let path_end = index + extension.len();
                value[path_end..].starts_with(':').then_some(path_end)
            })
        })
        .max()
}

pub(crate) fn compact_path(path: &str) -> String {
    let path = path
        .trim_matches('"')
        .strip_prefix("file://")
        .unwrap_or(path)
        .to_owned();
    normalize_path(&path)
}
