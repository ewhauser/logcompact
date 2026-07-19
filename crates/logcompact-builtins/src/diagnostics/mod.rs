mod arbitration;
mod common;
mod cpp;
mod go;
mod java;
mod javascript;
mod protobuf;
mod python;
mod rust;
mod typescript;

use std::collections::{BTreeMap, BTreeSet};

use crate::{Diagnostic, FallbackPolicy, normalize_terminal_text};

pub use go::parse_diagnostic as parse_go_diagnostic;
pub use java::JavaTestDiagnosticParser;
pub use javascript::JavaScriptTestDiagnosticParser;
pub use python::PythonDiagnosticParser;

/// Stable problem-matcher owners reserved by the built-in diagnostic pack.
pub const BUILTIN_MATCHER_OWNERS: &[&str] = &[
    "javascript-swc",
    "cpp-linker",
    "java-compiler",
    "rust-compiler",
    "javascript-test",
    "java-test",
    "python",
    "cpp",
    "typescript",
    "protobuf",
    "go",
];

/// Built-in diagnostic matchers replaced by custom matchers with the same owner.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BuiltinMatcherOverrides(u16);

impl BuiltinMatcherOverrides {
    #[must_use]
    pub fn from_owners<'a>(owners: impl IntoIterator<Item = &'a str>) -> Self {
        let mut overrides = Self::default();
        for owner in owners {
            overrides.insert(owner);
        }
        overrides
    }

    #[must_use]
    pub fn contains(self, owner: &str) -> bool {
        builtin_owner_bit(owner).is_some_and(|bit| self.0 & bit != 0)
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    fn insert(&mut self, owner: &str) {
        if let Some(bit) = builtin_owner_bit(owner) {
            self.0 |= bit;
        }
    }
}

fn builtin_owner_bit(owner: &str) -> Option<u16> {
    BUILTIN_MATCHER_OWNERS
        .iter()
        .position(|candidate| *candidate == owner)
        .map(|index| 1_u16 << index)
}

/// Static built-in reducer contract. Function pointers keep dispatch and
/// allocation costs identical regardless of how many parser modules exist.
#[derive(Clone, Copy)]
struct TextDiagnosticReducer {
    name: &'static str,
    enabled: fn(&TextDiagnosticHints) -> bool,
    reduce: for<'a> fn(&str, &mut TextDiagnosticContext<'a>),
}

#[derive(Clone, Copy)]
struct LineDiagnosticReducer {
    name: &'static str,
    enabled: fn(bool) -> bool,
    parse: fn(&str) -> Option<Diagnostic>,
}

struct TextDiagnosticContext<'a> {
    diagnostics: &'a mut Vec<Diagnostic>,
    swc_consumed_lines: BTreeSet<String>,
    has_swc_diagnostics: bool,
    javascript_test_messages: BTreeSet<String>,
    java_test_messages: BTreeSet<String>,
}

impl<'a> TextDiagnosticContext<'a> {
    fn new(diagnostics: &'a mut Vec<Diagnostic>) -> Self {
        Self {
            diagnostics,
            swc_consumed_lines: BTreeSet::new(),
            has_swc_diagnostics: false,
            javascript_test_messages: BTreeSet::new(),
            java_test_messages: BTreeSet::new(),
        }
    }
}

const TEXT_DIAGNOSTIC_REDUCERS: &[TextDiagnosticReducer] = &[
    TextDiagnosticReducer {
        name: "javascript-swc",
        enabled: may_contain_swc,
        reduce: reduce_swc,
    },
    TextDiagnosticReducer {
        name: "cpp-linker",
        enabled: may_contain_cpp_linker,
        reduce: reduce_cpp_linker,
    },
    TextDiagnosticReducer {
        name: "java-compiler",
        enabled: may_contain_java_compiler,
        reduce: reduce_java_compiler,
    },
    TextDiagnosticReducer {
        name: "rust-compiler",
        enabled: may_contain_rust_compiler,
        reduce: reduce_rust_compiler,
    },
    TextDiagnosticReducer {
        name: "javascript-test",
        enabled: may_contain_javascript_test,
        reduce: reduce_javascript_tests,
    },
    TextDiagnosticReducer {
        name: "java-test",
        enabled: may_contain_java_test,
        reduce: reduce_java_tests,
    },
    TextDiagnosticReducer {
        name: "python",
        enabled: may_contain_python,
        reduce: arbitration::reduce_python,
    },
];

const LINE_DIAGNOSTIC_REDUCERS: &[LineDiagnosticReducer] = &[
    LineDiagnosticReducer {
        name: "cpp",
        enabled: always,
        parse: cpp::parse_diagnostic,
    },
    LineDiagnosticReducer {
        name: "typescript",
        enabled: always,
        parse: typescript::parse_diagnostic,
    },
    LineDiagnosticReducer {
        name: "protobuf",
        enabled: always,
        parse: protobuf::parse_diagnostic,
    },
    LineDiagnosticReducer {
        name: "go",
        enabled: always,
        parse: go::parse_diagnostic,
    },
];

pub(crate) fn add_text_diagnostics(
    input: &[u8],
    diagnostics: &mut Vec<Diagnostic>,
    fallback: FallbackPolicy,
    overrides: BuiltinMatcherOverrides,
) {
    let normalized = normalize_terminal_text(input);
    add_normalized_text_diagnostics(&normalized, diagnostics, fallback, overrides);
}

pub(crate) fn add_normalized_text_diagnostics(
    normalized: &str,
    diagnostics: &mut Vec<Diagnostic>,
    fallback: FallbackPolicy,
    overrides: BuiltinMatcherOverrides,
) {
    let scan = TextDiagnosticScan::from_input(normalized);
    let mut context = TextDiagnosticContext::new(diagnostics);
    for reducer in TEXT_DIAGNOSTIC_REDUCERS {
        debug_assert!(!reducer.name.is_empty());
        if overrides.contains(reducer.name) {
            continue;
        }
        if !(reducer.enabled)(&scan.hints) {
            continue;
        }
        (reducer.reduce)(normalized, &mut context);
    }
    arbitration::reduce_lines(
        scan.candidates,
        &mut context,
        LINE_DIAGNOSTIC_REDUCERS,
        fallback == FallbackPolicy::Generic,
        overrides,
    );
}

#[derive(Default)]
struct TextDiagnosticHints {
    swc: bool,
    cpp_linker: bool,
    java_compiler: bool,
    rust_header: bool,
    rust_location: bool,
    javascript_exception: bool,
    javascript_location: bool,
    java_test: bool,
    python: bool,
}

struct TextDiagnosticScan<'a> {
    hints: TextDiagnosticHints,
    candidates: Vec<(&'a str, u32)>,
}

impl<'a> TextDiagnosticScan<'a> {
    fn from_input(input: &'a str) -> Self {
        let mut hints = TextDiagnosticHints::default();
        let mut counts = BTreeMap::<&str, u32>::new();
        let mut order = Vec::new();
        for raw_line in input.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }
            let classification = arbitration::classify_line(line);
            if classification.interesting {
                hints.swc |= line.contains(",-") || line.contains("╭─");
                hints.cpp_linker |= line.contains("undefined reference to ")
                    || line.contains("undefined symbol:")
                    || line.contains("unresolved external symbol ")
                    || line.contains("Undefined symbols for architecture ");
                hints.java_compiler |= line.contains(".java:");
                hints.rust_header |= line.contains("error[E");
                hints.rust_location |= line.contains("--> ");
                let exception = line.contains("Error") || line.contains("Exception");
                hints.javascript_exception |= exception;
                hints.javascript_location |= [
                    ".js:", ".jsx:", ".mjs:", ".cjs:", ".ts:", ".tsx:", ".mts:", ".cts:",
                ]
                .iter()
                .any(|extension| line.contains(extension));
                hints.java_test |=
                    exception || line.contains("Failure") || line.contains("Caused by: ");
                hints.python |= line.contains("Traceback (most recent call last):")
                    || line.contains("File \"")
                    || exception
                    || line.contains("Failure")
                    || line.contains("Warning")
                    || line.contains("KeyboardInterrupt")
                    || line.contains("SystemExit");
            } else {
                if line.starts_with("undefined reference to ")
                    || line.starts_with("unresolved external symbol ")
                {
                    hints.cpp_linker = true;
                }
            }
            if classification.candidate {
                if let Some(count) = counts.get_mut(line) {
                    *count = count.saturating_add(1);
                } else {
                    order.push(line);
                    counts.insert(line, 1);
                }
            }
        }
        let candidates = order
            .into_iter()
            .map(|line| (line, counts.get(line).copied().unwrap_or(1)))
            .collect();
        Self { hints, candidates }
    }
}

fn may_contain_swc(hints: &TextDiagnosticHints) -> bool {
    hints.swc
}

fn may_contain_cpp_linker(hints: &TextDiagnosticHints) -> bool {
    hints.cpp_linker
}

fn may_contain_java_compiler(hints: &TextDiagnosticHints) -> bool {
    hints.java_compiler
}

fn may_contain_rust_compiler(hints: &TextDiagnosticHints) -> bool {
    hints.rust_header && hints.rust_location
}

fn may_contain_javascript_test(hints: &TextDiagnosticHints) -> bool {
    hints.javascript_exception && hints.javascript_location
}

fn may_contain_java_test(hints: &TextDiagnosticHints) -> bool {
    hints.java_test
}

fn may_contain_python(hints: &TextDiagnosticHints) -> bool {
    hints.python
}

fn reduce_swc(input: &str, context: &mut TextDiagnosticContext<'_>) {
    let mut output = javascript::SwcParseOutput::default();
    javascript::reduce_swc(input, &mut output);
    context.has_swc_diagnostics = !output.diagnostics.is_empty();
    context.diagnostics.append(&mut output.diagnostics);
    context.swc_consumed_lines = output.consumed_lines;
}

fn reduce_cpp_linker(input: &str, context: &mut TextDiagnosticContext<'_>) {
    cpp::reduce_linker(input, context.diagnostics);
}

fn reduce_java_compiler(input: &str, context: &mut TextDiagnosticContext<'_>) {
    java::reduce_compiler(input, context.diagnostics);
}

fn reduce_rust_compiler(input: &str, context: &mut TextDiagnosticContext<'_>) {
    rust::reduce(input, context.diagnostics);
}

fn reduce_javascript_tests(input: &str, context: &mut TextDiagnosticContext<'_>) {
    javascript::reduce_tests(
        input,
        context.diagnostics,
        &mut context.javascript_test_messages,
    );
}

fn reduce_java_tests(input: &str, context: &mut TextDiagnosticContext<'_>) {
    java::reduce_tests(input, context.diagnostics, &mut context.java_test_messages);
}

fn always(_: bool) -> bool {
    true
}

#[cfg(feature = "test-support")]
pub(crate) fn parse_cpp_diagnostic(input: &str) -> Option<Diagnostic> {
    cpp::parse_diagnostic(input)
}

#[cfg(feature = "test-support")]
pub(crate) fn parse_cpp_linker_diagnostic(input: &str) -> Option<Diagnostic> {
    cpp::parse_linker_diagnostic(input)
}

#[cfg(feature = "test-support")]
pub(crate) fn cpp_path_end(input: &str, delimiter: char) -> Option<usize> {
    cpp::path_end(input, delimiter)
}

#[cfg(feature = "test-support")]
pub(crate) fn parse_swc_diagnostics(input: &str) -> Vec<Diagnostic> {
    javascript::parse_swc_diagnostics(input).diagnostics
}

#[cfg(feature = "test-support")]
pub(crate) fn parse_protobuf_diagnostic(input: &str) -> Option<Diagnostic> {
    protobuf::parse_diagnostic(input)
}

#[cfg(feature = "test-support")]
pub(crate) fn parse_typescript_diagnostic(input: &str) -> Option<Diagnostic> {
    typescript::parse_diagnostic(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contract_keeps_prepass_and_line_precedence_explicit() {
        assert_eq!(
            TEXT_DIAGNOSTIC_REDUCERS
                .iter()
                .map(|reducer| reducer.name)
                .collect::<Vec<_>>(),
            [
                "javascript-swc",
                "cpp-linker",
                "java-compiler",
                "rust-compiler",
                "javascript-test",
                "java-test",
                "python",
            ]
        );
        assert_eq!(
            LINE_DIAGNOSTIC_REDUCERS
                .iter()
                .map(|reducer| reducer.name)
                .collect::<Vec<_>>(),
            ["cpp", "typescript", "protobuf", "go"]
        );
        assert_eq!(
            BUILTIN_MATCHER_OWNERS,
            [
                "javascript-swc",
                "cpp-linker",
                "java-compiler",
                "rust-compiler",
                "javascript-test",
                "java-test",
                "python",
                "cpp",
                "typescript",
                "protobuf",
                "go",
            ]
        );
    }

    #[test]
    fn line_registry_preserves_parser_output_contract() {
        let mut diagnostics = Vec::new();
        add_text_diagnostics(
            b"src/looks_like.cc:7:3: error: first parser wins",
            &mut diagnostics,
            FallbackPolicy::Generic,
            BuiltinMatcherOverrides::default(),
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].location.as_ref().unwrap().path,
            "src/looks_like.cc"
        );
        assert_eq!(diagnostics[0].message, "first parser wins");
    }
}
