#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let normalized = tokencompact_builtins::normalize_terminal_text(data);
    let _ = tokencompact_builtins::deduplicate_lines(&normalized);
    let _ = tokencompact_builtins::reduce(
        &[tokencompact_builtins::TextInput::new(data)],
        &tokencompact_builtins::ReductionOptions {
            budget: tokencompact_builtins::Budget {
                max_bytes: 4 * 1024,
                max_items: 20,
            },
            ..tokencompact_builtins::ReductionOptions::default()
        },
        &tokencompact_builtins::NoRedaction,
    );

    let mut session = tokencompact_builtins::ReductionSession::new(
        tokencompact_builtins::builtin_parser_plan(
            tokencompact_builtins::BuiltinParserOptions::default(),
        ),
        tokencompact_builtins::SessionOptions {
            budget: tokencompact_builtins::Budget {
                max_bytes: 4 * 1024,
                max_items: 20,
            },
            limits: tokencompact_builtins::Limits {
                max_scope_bytes: 64 * 1024,
                max_line_bytes: 4 * 1024,
                max_candidates: 100,
                ..tokencompact_builtins::Limits::default()
            },
        },
        tokencompact_builtins::OutputPolicy::new(
            &tokencompact_builtins::NoRedaction,
            &tokencompact_builtins::NoPathMapping,
            &tokencompact_builtins::GenericRanker,
        ),
    );
    session.begin_scope(tokencompact_builtins::Scope::step("fuzz"));
    let chunk_size = data
        .first()
        .map_or(1, |byte| usize::from(*byte).saturating_add(1));
    for chunk in data.chunks(chunk_size) {
        session.push_chunk("fuzz", tokencompact_builtins::Stream::Combined, chunk);
    }
    session.end_scope("fuzz", tokencompact_builtins::EndReason::Complete);
    let _ = session.finish();
});
