use logcompact_builtins::{
    Budget, BuiltinParserOptions, EndReason, GenericRanker, NoPathMapping, NoRedaction,
    OutputPolicy, Reduction, ReductionSession, Scope, SessionOptions, Stream, builtin_parser_plan,
};

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

#[test]
fn test_state_is_chunk_invariant_and_scope_isolated() {
    let input = b"test invoice::fails ... FAILED\n---- invoice::fails stdout ----\nthread 'invoice::fails' panicked at src/lib.rs:7:3:\nassertion `left == right` failed\n";
    let expected = streaming(input, input.len());
    for chunk_size in 1..=32 {
        let actual = streaming(input, chunk_size);
        assert_eq!(expected.test_failures, actual.test_failures);
    }
}
