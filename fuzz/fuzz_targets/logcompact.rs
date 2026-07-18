#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let normalized = logcompact_builtins::normalize_terminal_text(data);
    let _ = logcompact_builtins::deduplicate_lines(&normalized);
    let _ = logcompact_builtins::reduce(
        &[logcompact_builtins::TextInput::new(data)],
        &logcompact_builtins::ReductionOptions {
            budget: logcompact_builtins::Budget {
                max_bytes: 4 * 1024,
                max_items: 20,
            },
            ..logcompact_builtins::ReductionOptions::default()
        },
        &logcompact_builtins::NoRedaction,
    );

    let mut session = logcompact_builtins::ReductionSession::new(
        logcompact_builtins::builtin_parser_plan(
            logcompact_builtins::BuiltinParserOptions::default(),
        ),
        logcompact_builtins::SessionOptions {
            budget: logcompact_builtins::Budget {
                max_bytes: 4 * 1024,
                max_items: 20,
            },
            limits: logcompact_builtins::Limits {
                max_scope_bytes: 64 * 1024,
                max_line_bytes: 4 * 1024,
                max_candidates: 100,
                ..logcompact_builtins::Limits::default()
            },
        },
        logcompact_builtins::OutputPolicy::new(
            &logcompact_builtins::NoRedaction,
            &logcompact_builtins::NoPathMapping,
            &logcompact_builtins::GenericRanker,
        ),
    );
    session.begin_scope(logcompact_builtins::Scope::step("fuzz"));
    let chunk_size = data
        .first()
        .map_or(1, |byte| usize::from(*byte).saturating_add(1));
    for chunk in data.chunks(chunk_size) {
        session.push_chunk("fuzz", logcompact_builtins::Stream::Combined, chunk);
    }
    session.end_scope("fuzz", logcompact_builtins::EndReason::Complete);
    let _ = session.finish();
});
