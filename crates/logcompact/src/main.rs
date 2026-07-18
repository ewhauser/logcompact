use std::error::Error;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use clap::{Parser as ClapParser, ValueEnum};
use logcompact::{OutputFormat, has_severity, render};
use logcompact_builtins::{
    Budget, BuiltinParserOptions, EndReason, GenericRanker, Limits, OutputPolicy, PathMapper,
    ProblemMatcherLimits, ProblemMatcherRegistry, Redactor, ReductionSession, Scope, ScopeKind,
    SessionOptions, Severity, Stream, builtin_parser_plan,
    builtin_parser_plan_with_problem_matchers,
};

#[derive(ClapParser, Debug)]
#[command(
    name = "logcompact",
    version,
    about = "Compact noisy logs into bounded, token-efficient diagnostics"
)]
struct Args {
    /// Input files. Omit or pass '-' to read stdin.
    #[arg(value_name = "INPUT")]
    inputs: Vec<PathBuf>,

    #[arg(long, value_enum, default_value_t = FormatArg::Human)]
    format: FormatArg,

    #[arg(long, default_value = "log")]
    scope_id: String,

    #[arg(long, value_enum, default_value_t = ScopeKindArg::Command)]
    scope_kind: ScopeKindArg,

    #[arg(long, value_enum, default_value_t = StreamArg::Combined)]
    stream: StreamArg,

    #[arg(long, default_value_t = 20)]
    max_items: usize,

    #[arg(long, default_value_t = 64 * 1024)]
    max_output_bytes: usize,

    #[arg(long, default_value_t = 16 * 1024 * 1024)]
    max_input_bytes: usize,

    #[arg(long, default_value_t = 64 * 1024)]
    max_line_bytes: usize,

    /// Literal output text to replace with `[REDACTED]`. Repeat as needed.
    #[arg(long)]
    redact_literal: Vec<String>,

    /// Path prefix to remove after parsing. Repeat for CI workspace variants.
    #[arg(long)]
    strip_prefix: Vec<String>,

    /// GitHub/VS Code matcher JSON. Later owners replace earlier or same-owner built-ins.
    #[arg(long, value_name = "FILE")]
    problem_matcher: Vec<PathBuf>,

    /// Exit with status 1 when a finding at this severity or higher exists.
    #[arg(long, value_enum)]
    fail_on: Option<SeverityArg>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum FormatArg {
    Human,
    Json,
    Jsonl,
    Sarif,
    Github,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ScopeKindArg {
    Invocation,
    Job,
    Step,
    Command,
    Test,
    Attempt,
    Other,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum StreamArg {
    Stdout,
    Stderr,
    Combined,
    Annotation,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum SeverityArg {
    Error,
    Warning,
    Note,
}

struct LiteralRedactor(Vec<String>);

impl Redactor for LiteralRedactor {
    fn redact(&self, value: &str) -> String {
        self.0.iter().fold(value.to_owned(), |output, literal| {
            output.replace(literal, "[REDACTED]")
        })
    }
}

struct PrefixPathMapper(Vec<String>);

impl PathMapper for PrefixPathMapper {
    fn map_path(&self, value: &str) -> String {
        self.0
            .iter()
            .find_map(|prefix| value.strip_prefix(prefix))
            .map(|relative| relative.trim_start_matches(['/', '\\']).to_owned())
            .unwrap_or_else(|| value.to_owned())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    if args.redact_literal.iter().any(String::is_empty) {
        return Err("--redact-literal must not be empty".into());
    }
    let redactor = LiteralRedactor(args.redact_literal.clone());
    let path_mapper = PrefixPathMapper(args.strip_prefix.clone());
    let ranker = GenericRanker;
    let parser_options = BuiltinParserOptions {
        max_buffered_scope_bytes: args.max_input_bytes,
        ..BuiltinParserOptions::default()
    };
    let parser_plan = load_problem_matchers(&args.problem_matcher)?.map_or_else(
        || builtin_parser_plan(parser_options),
        |registry| builtin_parser_plan_with_problem_matchers(parser_options, registry),
    );
    let mut session = ReductionSession::new(
        parser_plan,
        SessionOptions {
            budget: Budget {
                max_bytes: args.max_output_bytes,
                max_items: args.max_items,
            },
            limits: Limits {
                max_scopes: args.inputs.len().max(1),
                max_scope_bytes: args.max_input_bytes,
                max_line_bytes: args.max_line_bytes,
                max_candidates: args.max_items.saturating_mul(100).max(1_000),
            },
        },
        OutputPolicy::new(&redactor, &path_mapper, &ranker),
    );

    let inputs = if args.inputs.is_empty() {
        vec![PathBuf::from("-")]
    } else {
        args.inputs.clone()
    };
    let mut stdin_consumed = false;
    for (index, input) in inputs.iter().enumerate() {
        let scope_id = if inputs.len() == 1 {
            args.scope_id.clone()
        } else {
            format!("{}-{index}", args.scope_id)
        };
        let label = input.to_string_lossy().into_owned();
        let scope = Scope::new(scope_id.clone(), args.scope_kind.into()).with_label(label);
        if !session.begin_scope(scope) {
            return Err(format!("could not begin scope {scope_id:?}").into());
        }
        let read_result = if input.as_os_str() == "-" {
            if stdin_consumed {
                return Err("stdin may only be supplied once".into());
            }
            stdin_consumed = true;
            stream_reader(
                &mut io::stdin().lock(),
                &mut session,
                &scope_id,
                args.stream.into(),
            )
        } else {
            let mut file = File::open(input)?;
            stream_reader(&mut file, &mut session, &scope_id, args.stream.into())
        };
        session.end_scope(
            &scope_id,
            if read_result.is_ok() {
                EndReason::Complete
            } else {
                EndReason::Interrupted
            },
        );
        if let Err(error) = read_result {
            return Err(format!("could not read {}: {error}", input.display()).into());
        }
    }

    let reduction = session.finish();
    io::stdout()
        .lock()
        .write_all(render(&reduction, args.format.into()).as_bytes())?;
    if args
        .fail_on
        .is_some_and(|minimum| has_severity(&reduction, minimum.into()))
    {
        std::process::exit(1);
    }
    Ok(())
}

fn load_problem_matchers(
    paths: &[PathBuf],
) -> Result<Option<ProblemMatcherRegistry>, Box<dyn Error>> {
    if paths.is_empty() {
        return Ok(None);
    }
    let limits = ProblemMatcherLimits::default();
    if paths.len() > limits.max_matchers {
        return Err(format!(
            "{} problem matcher files were supplied; maximum is {}",
            paths.len(),
            limits.max_matchers
        )
        .into());
    }
    let mut registry = ProblemMatcherRegistry::new(limits);
    for path in paths {
        let mut definition = Vec::new();
        File::open(path)?
            .take((limits.max_document_bytes as u64).saturating_add(1))
            .read_to_end(&mut definition)?;
        registry.add_json(&definition).map_err(|error| {
            format!("could not load problem matcher {}: {error}", path.display())
        })?;
    }
    Ok(Some(registry))
}

fn stream_reader(
    reader: &mut dyn Read,
    session: &mut ReductionSession<'_>,
    scope_id: &str,
    stream: Stream,
) -> io::Result<()> {
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        if !session.push_chunk(scope_id, stream, &buffer[..read]) {
            return Err(io::Error::other("reduction scope disappeared"));
        }
    }
}

impl From<FormatArg> for OutputFormat {
    fn from(value: FormatArg) -> Self {
        match value {
            FormatArg::Human => Self::Human,
            FormatArg::Json => Self::Json,
            FormatArg::Jsonl => Self::JsonLines,
            FormatArg::Sarif => Self::Sarif,
            FormatArg::Github => Self::GitHub,
        }
    }
}

impl From<ScopeKindArg> for ScopeKind {
    fn from(value: ScopeKindArg) -> Self {
        match value {
            ScopeKindArg::Invocation => Self::Invocation,
            ScopeKindArg::Job => Self::Job,
            ScopeKindArg::Step => Self::Step,
            ScopeKindArg::Command => Self::Command,
            ScopeKindArg::Test => Self::Test,
            ScopeKindArg::Attempt => Self::Attempt,
            ScopeKindArg::Other => Self::Other,
        }
    }
}

impl From<StreamArg> for Stream {
    fn from(value: StreamArg) -> Self {
        match value {
            StreamArg::Stdout => Self::Stdout,
            StreamArg::Stderr => Self::Stderr,
            StreamArg::Combined => Self::Combined,
            StreamArg::Annotation => Self::Annotation,
        }
    }
}

impl From<SeverityArg> for Severity {
    fn from(value: SeverityArg) -> Self {
        match value {
            SeverityArg::Error => Self::Error,
            SeverityArg::Warning => Self::Warning,
            SeverityArg::Note => Self::Note,
        }
    }
}
