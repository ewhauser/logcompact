use std::fs;
use std::path::{Path, PathBuf};

use logcompact_builtins::{
    Budget, BuiltinParserOptions, Diagnostic, EndReason, GenericRanker, NoPathMapping, NoRedaction,
    OutputPolicy, Reduction, ReductionOptions, ReductionSession, Scope, SessionOptions, Stream,
    TextInput, builtin_parser_plan, reduce,
};
use serde::Deserialize;

const MAX_CASE_BYTES: u64 = 64 * 1024;
const MAX_CASES: usize = 1_024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticTestCase {
    output: String,
    assertion: DiagnosticAssertion,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticAssertion {
    diagnostics: Vec<Diagnostic>,
}

#[test]
fn diagnostic_testcases_match_batch_and_streaming_reduction() {
    let cases = discover_testcases();
    assert!(!cases.is_empty(), "the diagnostic testcase corpus is empty");
    assert!(
        cases.len() <= MAX_CASES,
        "diagnostic testcase corpus exceeds its {MAX_CASES}-case bound"
    );

    for path in cases {
        run_testcase(&path);
    }
}

fn discover_testcases() -> Vec<PathBuf> {
    let diagnostics = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/diagnostics");
    let mut languages = directories(&diagnostics);
    languages.sort();

    let mut cases = Vec::new();
    for language in languages {
        let testcases = language.join("testcases");
        assert!(
            testcases.is_dir(),
            "language module {} is missing a testcases directory",
            language.display()
        );

        let mut frameworks = directories(&testcases);
        frameworks.sort();
        assert!(
            !frameworks.is_empty(),
            "{} has no framework directories",
            testcases.display()
        );

        for framework in frameworks {
            let mut framework_cases = yaml_files(&framework);
            framework_cases.sort();
            assert!(
                !framework_cases.is_empty(),
                "framework directory {} has no YAML cases",
                framework.display()
            );
            cases.extend(framework_cases);
        }
    }
    cases
}

fn directories(path: &Path) -> Vec<PathBuf> {
    fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
        .map(|entry| {
            entry.unwrap_or_else(|error| panic!("failed to read {} entry: {error}", path.display()))
        })
        .filter_map(|entry| {
            entry
                .file_type()
                .unwrap_or_else(|error| {
                    panic!("failed to inspect {}: {error}", entry.path().display())
                })
                .is_dir()
                .then(|| entry.path())
        })
        .collect()
}

fn yaml_files(path: &Path) -> Vec<PathBuf> {
    fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
        .map(|entry| {
            entry.unwrap_or_else(|error| panic!("failed to read {} entry: {error}", path.display()))
        })
        .filter_map(|entry| {
            let path = entry.path();
            (entry
                .file_type()
                .unwrap_or_else(|error| panic!("failed to inspect {}: {error}", path.display()))
                .is_file()
                && matches!(
                    path.extension().and_then(|value| value.to_str()),
                    Some("yaml")
                ))
            .then_some(path)
        })
        .collect()
}

fn run_testcase(path: &Path) {
    let metadata = fs::metadata(path)
        .unwrap_or_else(|error| panic!("failed to inspect {}: {error}", path.display()));
    assert!(
        metadata.len() <= MAX_CASE_BYTES,
        "{} exceeds the {MAX_CASE_BYTES}-byte testcase bound",
        path.display()
    );
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let case = serde_saphyr::from_str::<DiagnosticTestCase>(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()));
    let output = case.output.as_bytes();
    let batch = reduce(
        &[TextInput::new(output)],
        &ReductionOptions {
            budget: Budget::unbounded(),
            ..ReductionOptions::default()
        },
        &NoRedaction,
    );

    assert_eq!(
        case.assertion.diagnostics,
        batch.diagnostics,
        "batch assertion failed for {}",
        path.display()
    );

    for chunk_size in chunk_sizes(output.len()) {
        let streamed = without_provenance(streaming(output, chunk_size));
        assert_eq!(
            batch.diagnostics,
            streamed.diagnostics,
            "chunk size {chunk_size} changed the reduction for {}",
            path.display()
        );
    }
}

fn chunk_sizes(input_len: usize) -> impl Iterator<Item = usize> {
    let upper = input_len.clamp(1, 64);
    1..=upper
}

fn streaming(input: &[u8], chunk_size: usize) -> Reduction {
    let mut session = ReductionSession::new(
        builtin_parser_plan(BuiltinParserOptions::default()),
        SessionOptions {
            budget: Budget::unbounded(),
            ..SessionOptions::default()
        },
        OutputPolicy::new(&NoRedaction, &NoPathMapping, &GenericRanker),
    );
    session.begin_scope(Scope::step("fixture"));
    for chunk in input.chunks(chunk_size) {
        session.push_chunk("fixture", Stream::Stderr, chunk);
    }
    session.end_scope("fixture", EndReason::Complete);
    session.finish()
}

fn without_provenance(mut reduction: Reduction) -> Reduction {
    for diagnostic in &mut reduction.diagnostics {
        diagnostic.provenance = None;
    }
    reduction.stats = Default::default();
    reduction
}
