# tokencompact-builtins

`tokencompact-builtins` is the provider-neutral built-in parser pack for
`tokencompact-core`. It recognizes common compiler, linter, runtime, and
test output without requiring a CI provider, build system, process runner,
database, or async runtime.

The repository develops three publishable packages together:

| Package | Responsibility |
| --- | --- |
| `tokencompact-core` | Streaming scopes, parser lifecycle, provenance, policies, bounds, and finalization |
| `tokencompact-builtins` | Fixed built-in language and test-framework parser pack plus a batch compatibility API |
| `tokencompact` | Incremental file/stdin input and human, JSON, JSONL, SARIF, and GitHub presentation |

## Batch API

```rust
use tokencompact_builtins::{Budget, NoRedaction, ReductionOptions, TextInput, reduce};

let result = reduce(
    &[TextInput::new(b"src/main.go:12:4: undefined: total")],
    &ReductionOptions {
        budget: Budget { max_bytes: 4096, max_items: 20 },
        ..ReductionOptions::default()
    },
    &NoRedaction,
);

assert_eq!(result.diagnostics[0].message, "undefined: total");
assert_eq!(result.diagnostics[0].location.as_ref().unwrap().path, "src/main.go");
```

## Streaming API

```rust
use tokencompact_builtins::{
    Budget, BuiltinParserOptions, EndReason, GenericRanker, NoPathMapping,
    NoRedaction, OutputPolicy, ReductionSession, Scope, SessionOptions, Stream,
    builtin_parser_plan,
};

let mut session = ReductionSession::new(
    builtin_parser_plan(BuiltinParserOptions::default()),
    SessionOptions {
        budget: Budget { max_bytes: 4096, max_items: 20 },
        ..SessionOptions::default()
    },
    OutputPolicy::new(&NoRedaction, &NoPathMapping, &GenericRanker),
);
session.begin_scope(Scope::step("compile"));
session.push_chunk("compile", Stream::Stderr, b"src/main.go:12:");
session.push_chunk("compile", Stream::Stderr, b"4: undefined: total\n");
session.end_scope("compile", EndReason::Complete);
let result = session.finish();
```

Chunk boundaries do not affect results. Each scope has an explicit end reason,
and incomplete test segments do not emit structured blocks whose confirmation
may have been truncated. The built-in plan has a fixed parser order; runtime
plugin discovery is not part of the contract.

Paths are mapped before strings are redacted. Redaction covers messages, paths,
test names, framework names, and provenance before exact deduplication, ranking,
serialized-byte accounting, or return. Production consumers should normally
provide a `Redactor` instead of `NoRedaction`.

The generic pack does not understand provider workspaces, action mnemonics,
build-event protocols, or provider-specific status messages. Those rules belong
in downstream adapters, which can feed mapped findings through the same
output-policy boundary.

Run `make boundary` from the repository root to verify that no provider-specific
semantics or workspace-internal dependencies have leaked into these packages.
