# Agent instructions

This repository reduces noisy logs into deterministic, token-efficient
findings. Preserve these invariants:

- Keep the dependency direction `logcompact` -> `logcompact-builtins` ->
  `logcompact-core`.
- Keep the core and built-in reducers synchronous, deterministic, bounded, and
  independent of providers, build systems, processes, filesystems, networks,
  environment variables, clocks, randomness, and async runtimes.
- Keep parser plans immutable after input begins and parser order stable.
- Preserve chunk-invariant CR, LF, and CRLF framing.
- Map paths and redact all model-visible fields before deduplication, ranking,
  serialization, telemetry, or return.
- Preserve explicit bounds for retained input, line length, scopes,
  candidates, output items, and serialized bytes.
- Keep the CLI a thin incremental I/O and presentation adapter. It does not
  launch commands or invoke a shell.
- Every diagnostic bug fix and every new diagnostic or framework must add or
  update a YAML regression case at
  `crates/logcompact-builtins/src/diagnostics/<language>/testcases/<framework>/<case>.yaml`.
  The corpus harness discovers these `.yaml` files automatically; cases must
  contain exactly `output` and `assertion` sections and must pass both exact
  batch assertions and streaming chunk-equivalence checks.
- Treat fixture changes as reviewed golden diffs.
- Keep all crates publishable and use Conventional Commits.

Run `make check`, `make fuzz-smoke`, and `make package-check` before merging.
Do not run long benchmarks as ordinary tests.
