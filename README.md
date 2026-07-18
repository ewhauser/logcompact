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
logcompact build.log --problem-matcher .github/compiler.json
```

The output limit is measured in serialized bytes. That is deterministic across
machines and model providers; it is a stable proxy for token cost, not a claim
to reproduce any particular tokenizer.

## Performance

Logcompact is designed to sit in a log pipeline without becoming the
bottleneck. Representative Criterion medians on GitHub Actions
`ubuntu-latest` at `v0.3.0`:

| Workload | Input | Median | Throughput |
| --- | ---: | ---: | ---: |
| Exact deduplication, repeated lines | 100,000 lines / 1.80 MB | 2.22 ms | 809 MB/s |
| Core streaming framing, 64 KiB chunks | 25,000 lines / 814 KB | 7.70 ms | 106 MB/s |
| Single custom matcher, sparse matches, 64 KiB chunks | 25,000 lines / 1.01 MB | 13.3 ms | 76.1 MB/s |
| Full built-in parser pack, mixed log, 64 KiB chunks | 25,000 lines / 1.01 MB | 160 ms | 6.32 MB/s |

Even the full parser pack reduces a roughly 1 MB, 25,000-line log in about
160 ms on a hosted CI runner. These benchmarks measure in-memory reduction;
filesystem and terminal I/O are excluded. See the
[benchmark run](https://github.com/ewhauser/logcompact/actions/runs/29664825162)
and [benchmark methodology](BENCHMARKS.md), or reproduce them with
`make bench`.

## Crates

The dependency direction is deliberately one-way:

| Package | Responsibility |
| --- | --- |
| [`logcompact-core`](crates/logcompact-core) | Synchronous streaming state machine, scopes, parser lifecycle, provenance, redaction, ranking, and budgets |
| [`logcompact-builtins`](crates/logcompact-builtins) | Fixed parser pack for common compiler, test, lint, and runtime output |
| [`logcompact`](crates/logcompact) | Incremental stdin/file adapter and human, JSON, JSONL, SARIF, and GitHub output |

`logcompact-core` performs no I/O and knows nothing about CI providers,
command runners, storage, or build systems. Applications can use the built-in
pack, compile self-contained GitHub/VS Code problem matcher definitions, supply
their own `Parser` implementations, or feed trusted structured findings into
the same output-policy boundary.

See [PROBLEM_MATCHERS.md](PROBLEM_MATCHERS.md) for the supported matcher
contract, configuration examples, precedence with built-ins, performance
guidance, safety bounds, and deliberate compatibility limits. CLI matchers
extend the built-ins by default. A custom matcher whose `owner` equals a stable
built-in owner replaces that built-in for the invocation.

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
