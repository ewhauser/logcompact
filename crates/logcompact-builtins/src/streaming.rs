use std::collections::BTreeMap;

use logcompact_core::{
    Emitter, EndReason, EvidenceQuality, FallbackPolicy, LogLine, Parser, ParserPlan, Provenance,
    Scope, ScopeBoundary, Stream,
};

use crate::diagnostics::{self, BuiltinMatcherOverrides};
use crate::test_log::TestLogReducer;

/// Fixed options for the built-in parser pack in a streaming session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuiltinParserOptions {
    pub fallback: FallbackPolicy,
    pub max_buffered_scope_bytes: usize,
    pub overrides: BuiltinMatcherOverrides,
}

impl Default for BuiltinParserOptions {
    fn default() -> Self {
        Self {
            fallback: FallbackPolicy::Generic,
            max_buffered_scope_bytes: 1024 * 1024,
            overrides: BuiltinMatcherOverrides::default(),
        }
    }
}

/// Creates the immutable, versioned built-in parser order.
#[must_use]
pub fn builtin_parser_plan(options: BuiltinParserOptions) -> ParserPlan {
    let mut plan = ParserPlan::new();
    plan.push(BuiltinDiagnosticParser::new(options))
        .expect("the built-in diagnostic parser identifier is unique");
    plan.push(TestFailureParser::default())
        .expect("the built-in test parser identifier is unique");
    plan
}

#[derive(Clone, Debug)]
struct BufferedStream {
    text: String,
    first_line: u64,
    last_line: u64,
    truncated: bool,
}

/// Bounded bridge from incremental normalized lines to the built-in parser
/// registry. Individual language parsers retain their deterministic prepass and
/// line-arbitration order while callers gain chunk-invariant session framing.
pub struct BuiltinDiagnosticParser {
    options: BuiltinParserOptions,
    scopes: BTreeMap<String, BTreeMap<Stream, BufferedStream>>,
}

impl BuiltinDiagnosticParser {
    #[must_use]
    pub fn new(options: BuiltinParserOptions) -> Self {
        Self {
            options,
            scopes: BTreeMap::new(),
        }
    }

    fn flush_scope(&mut self, scope: &Scope, emitter: &mut Emitter<'_>) {
        let Some(streams) = self.scopes.remove(&scope.id) else {
            return;
        };
        for (stream, buffer) in streams {
            let mut parsed = Vec::new();
            diagnostics::add_normalized_text_diagnostics(
                &buffer.text,
                &mut parsed,
                self.options.fallback,
                self.options.overrides,
            );
            for mut diagnostic in parsed {
                if diagnostic.location.is_some() {
                    diagnostic.quality = EvidenceQuality::Located;
                }
                diagnostic.provenance = Some(
                    Provenance::new(stream_name(stream))
                        .with_scope(scope)
                        .with_span(stream, buffer.first_line, buffer.last_line)
                        .with_parser(self.id()),
                );
                emitter.diagnostic(diagnostic);
            }
        }
    }
}

impl Parser for BuiltinDiagnosticParser {
    fn id(&self) -> &'static str {
        "builtin.diagnostics.v1"
    }

    fn observe(&mut self, line: &LogLine<'_>, _emitter: &mut Emitter<'_>) {
        if !self.scopes.contains_key(&line.scope.id) {
            self.scopes.insert(line.scope.id.clone(), BTreeMap::new());
        }
        let streams = self
            .scopes
            .get_mut(&line.scope.id)
            .expect("scope buffer was inserted above");
        let buffer = streams
            .entry(line.stream)
            .or_insert_with(|| BufferedStream {
                text: String::new(),
                first_line: line.stream_line,
                last_line: line.stream_line,
                truncated: false,
            });
        buffer.last_line = line.stream_line;
        let required = line.text.len().saturating_add(1);
        let available = self
            .options
            .max_buffered_scope_bytes
            .saturating_sub(buffer.text.len());
        if required <= available {
            if !buffer.text.is_empty() {
                buffer.text.push('\n');
            }
            buffer.text.push_str(line.text);
        } else {
            buffer.truncated = true;
            let retained = available.min(line.text.len());
            let mut boundary = retained;
            while boundary > 0 && !line.text.is_char_boundary(boundary) {
                boundary -= 1;
            }
            if boundary > 0 {
                if !buffer.text.is_empty() {
                    buffer.text.push('\n');
                }
                buffer.text.push_str(&line.text[..boundary]);
            }
        }
        buffer.truncated |= line.truncated;
    }

    fn boundary(&mut self, boundary: ScopeBoundary<'_>, emitter: &mut Emitter<'_>) {
        if let ScopeBoundary::End { scope, .. } = boundary {
            self.flush_scope(scope, emitter);
        }
    }
}

#[derive(Default)]
struct TestFailureParser {
    scopes: BTreeMap<String, TestLogReducer>,
}

impl Parser for TestFailureParser {
    fn id(&self) -> &'static str {
        "builtin.test-log.v1"
    }

    fn observe(&mut self, line: &LogLine<'_>, _emitter: &mut Emitter<'_>) {
        if self.scopes.is_empty() {
            if !TestLogReducer::may_start_failure_line(line.text) {
                return;
            }
        } else {
            let active = self
                .scopes
                .get(&line.scope.id)
                .is_some_and(TestLogReducer::failure_state_active);
            if !active && !TestLogReducer::may_start_failure_line(line.text) {
                return;
            }
        }
        let provenance = Provenance::new(stream_name(line.stream))
            .with_scope(line.scope)
            .with_span(line.stream, line.stream_line, line.stream_line)
            .with_parser(self.id());
        self.scopes
            .entry(line.scope.id.clone())
            .or_default()
            .observe_failure_line(line.text, &provenance);
    }

    fn boundary(&mut self, boundary: ScopeBoundary<'_>, emitter: &mut Emitter<'_>) {
        let ScopeBoundary::End { scope, reason, .. } = boundary else {
            return;
        };
        let Some(mut reducer) = self.scopes.remove(&scope.id) else {
            return;
        };
        reducer.finish_log(reason == EndReason::Complete);
        let result = reducer.finish();
        for failure in result.failures {
            emitter.test_failure(failure);
        }
    }
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

#[cfg(test)]
mod tests {
    use logcompact_core::{
        Budget, EndReason, GenericRanker, NoPathMapping, NoRedaction, OutputPolicy,
        ReductionSession, Scope, SessionOptions,
    };

    use super::*;

    fn reduce_chunks(chunks: &[&[u8]]) -> logcompact_core::Reduction {
        let mut session = ReductionSession::new(
            builtin_parser_plan(BuiltinParserOptions::default()),
            SessionOptions {
                budget: Budget::unbounded(),
                ..SessionOptions::default()
            },
            OutputPolicy::new(&NoRedaction, &NoPathMapping, &GenericRanker),
        );
        session.begin_scope(Scope::step("tests"));
        for chunk in chunks {
            session.push_chunk("tests", Stream::Stderr, chunk);
        }
        session.end_scope("tests", EndReason::Complete);
        session.finish()
    }

    #[test]
    fn batch_and_arbitrary_stream_chunks_are_equivalent() {
        let text = b"error[E0308]: mismatched types\n --> src/lib.rs:7:5\n  |\n7 | value\n  | ^ expected u32, found &str\n";
        let whole = reduce_chunks(&[text]);
        let split = reduce_chunks(&[&text[..9], &text[9..31], &text[31..]]);
        assert_eq!(whole.diagnostics, split.diagnostics);
        assert_eq!(
            whole.diagnostics[0].location.as_ref().unwrap().path,
            "src/lib.rs"
        );
    }

    #[test]
    fn emits_structured_test_failures_for_each_streaming_framework() {
        for (input, expected_name) in [
            (
                &b"test invoice::fails ... FAILED\n---- invoice::fails stdout ----\nthread 'invoice::fails' panicked at src/lib.rs:7:3:\nassertion `left == right` failed\n"[..],
                "invoice::fails",
            ),
            (
                &b"[ RUN      ] InvoiceTest.Fails\nsrc/invoice_test.cc:12: Failure\nExpected equality of these values\n[  FAILED  ] InvoiceTest.Fails (1 ms)\n"[..],
                "InvoiceTest.Fails",
            ),
            (
                &b"=== RUN   TestInvoice\ninvoice_test.go:7: got 2; want 3\n--- FAIL: TestInvoice (0.00s)\n"[..],
                "TestInvoice",
            ),
        ] {
            let reduction = reduce_chunks(&[input]);
            assert_eq!(reduction.test_failures.len(), 1, "{expected_name}");
            assert_eq!(reduction.test_failures[0].name, expected_name);
        }
    }
}
