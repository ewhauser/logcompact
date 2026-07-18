# logcompact

Keep the signal. Drop the tokens.

`logcompact` deterministically compacts compiler, test, lint, runtime, and
tool logs into bounded structured findings. It is designed for AI agents, CI
annotations, and command-line workflows where raw logs are useful evidence but
an expensive interface.

```console
cargo install logcompact

some-command 2>&1 | logcompact
logcompact build.log --format json --max-output-bytes 8192
logcompact test.log --format github --fail-on error
logcompact build.log --format sarif > diagnostics.sarif
```

The output limit is measured in serialized bytes. That is deterministic across
machines and model providers; it is a stable proxy for token cost, not a claim
to reproduce any particular tokenizer.

## Crates

The dependency direction is deliberately one-way:

| Package | Responsibility |
| --- | --- |
| [`logcompact-core`](crates/logcompact-core) | Synchronous streaming state machine, scopes, parser lifecycle, provenance, redaction, ranking, and budgets |
| [`logcompact-builtins`](crates/logcompact-builtins) | Fixed parser pack for common compiler, test, lint, and runtime output |
| [`logcompact`](crates/logcompact) | Incremental stdin/file adapter and human, JSON, JSONL, SARIF, and GitHub output |

`logcompact-core` performs no I/O and knows nothing about CI providers,
command runners, storage, or build systems. Applications can use the built-in
pack, supply their own `Parser` implementations, or feed trusted structured
findings into the same output-policy boundary.

## Deterministic by construction

- Chunk boundaries do not change results.
- Parser order is fixed; there is no runtime plugin discovery.
- Retained input, line length, open scopes, candidates, output items, and
  serialized output are explicitly bounded.
- Path mapping and redaction happen before deduplication, ranking, budgeting,
  and return.
- No raw logs are retained by the library. Callers own evidence retention.
- Reducers use no filesystem, environment, network, clock, randomness, or
  async runtime.

## CLI safety and output

Input is read incrementally from stdin or explicit files. The CLI does not
launch commands or invoke a shell. Use `--redact-literal` to replace sensitive
text and `--strip-prefix` to project workspace paths without filesystem access.

Formats:

- `human` for a compact terminal summary;
- `json` for one complete `Reduction` document;
- `jsonl` for streaming-friendly finding records and a final summary;
- `sarif` for code-scanning integrations;
- `github` for GitHub Actions workflow commands.

Run `logcompact --help` for limits, scope metadata, stream selection, and
failure-threshold options.

## Development

Rust 1.94.1 is the minimum supported version.

```console
make check
make fuzz-smoke
make package-check
```

See [BENCHMARKS.md](BENCHMARKS.md) for benchmark ownership and commands,
[CONTRIBUTING.md](CONTRIBUTING.md) for contribution rules, and
[RELEASING.md](RELEASING.md) for the dependency-ordered crates.io process.

Licensed under the [MIT License](LICENSE).
