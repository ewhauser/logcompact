# Benchmarks

Logcompact owns the performance contract from input bytes to bounded,
structured diagnostics. Provider and build-system integrations should keep
their adapter, transport, storage, and end-to-end benchmarks in their own
repositories.

## Suites

| Suite | Coverage | Command |
| --- | --- | --- |
| Core | Exact deduplication at 100, 10,000, and 100,000 repeated or unique lines; CR, LF, and CRLF framing with 7-byte, 1 KiB, and 64 KiB chunks | `make bench-core` |
| Built-ins | No-match, mixed, match-heavy, and repeated diagnostics through batch and streaming reduction; matcher-only, built-ins-only, and combined problem-matcher plans | `make bench-builtins` |

`make bench` runs both suites. Pull-request CI compiles benchmarks without
running measurements. A scheduled and manually dispatched GitHub Actions
workflow runs Criterion on a consistent runner class and retains its reports
as workflow artifacts for 30 days.

Criterion results are meaningful only when the candidate and baseline run on
comparable hardware. Do not gate pull requests using measurements collected
on different runner classes. Before moving or replacing a benchmark, run the
old and new forms on the same machine and record the case mapping in the
change description.

## Migration from Bazel MCP

The original Bazel MCP `reduction/deduplicate` cases map to
`core/deduplication/repeated`. Logcompact expands those cases with unique-line
inputs, provider-neutral framing, match-heavy parsing, repeated findings, and
adversarial chunk boundaries.

Bazel query reduction, Starlark reducer comparisons, storage, BEP/BES,
model-visible token, MCP latency, and agentic benchmarks remain in Bazel MCP
because they measure Bazel-specific integration behavior.
