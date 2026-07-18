# logcompact-builtins

`logcompact-builtins` is the provider-neutral built-in parser pack for
`logcompact-core`. It recognizes common compiler, linter, runtime, and
test output without requiring a CI provider, build system, process runner,
database, or async runtime.

The repository develops three publishable packages together:

| Package | Responsibility |
| --- | --- |
| `logcompact-core` | Streaming scopes, parser lifecycle, provenance, policies, bounds, and finalization |
| `logcompact-builtins` | Fixed language/test parser pack, bounded problem matcher compiler, and batch compatibility API |
| `logcompact` | Incremental file/stdin input and human, JSON, JSONL, SARIF, and GitHub presentation |

## Batch API

```rust
use logcompact_builtins::{Budget, NoRedaction, ReductionOptions, TextInput, reduce};

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
use logcompact_builtins::{
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

## Problem matcher API

Self-contained GitHub Actions and inline VS Code problem matcher definitions
can be compiled before input begins and appended to a parser plan:

```rust
use logcompact_builtins::{
    BuiltinParserOptions, ProblemMatcherRegistry, builtin_parser_plan,
};

let mut registry = ProblemMatcherRegistry::default();
registry.add_json(br#"{
    "problemMatcher": [{
        "owner": "compiler",
        "pattern": {
            "regexp": "^(.+):(\\d+):(\\d+): (error|warning): (.+)$",
            "file": 1, "line": 2, "column": 3,
            "severity": 4, "message": 5
        }
    }]
}"#).expect("the matcher definition should compile");

let mut plan = builtin_parser_plan(BuiltinParserOptions::default());
plan.push(registry.into_parser()).expect("the parser id is unique");
```

The registry performs no I/O. It validates all definitions and compiles their
bounded state machines atomically. See the repository's
[`PROBLEM_MATCHERS.md`](../../PROBLEM_MATCHERS.md) for compatibility details.

Run `make boundary` from the repository root to verify that no provider-specific
semantics or workspace-internal dependencies have leaked into these packages.
