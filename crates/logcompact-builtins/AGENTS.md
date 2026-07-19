# LogCompact built-ins instructions

This crate is source-agnostic, synchronous, deterministic, and bounded. It must
not depend on Bazel protocol/domain crates, MCP, storage, the command runner, an
async runtime, the filesystem, environment variables, the network, clocks, or
randomness. Preserve parser order, exact deduplication, stable ordering, control
sanitization, redaction-before-return, and serialized diagnostic byte budgets.

Every diagnostic bug fix and every new diagnostic or framework must include a
YAML regression case under
`src/diagnostics/<language>/testcases/<framework>/<case>.yaml`. The integration
harness discovers direct `.yaml` children of every framework directory
automatically, so adding a correctly placed case requires no Rust registration.
Each case must contain exactly `output` and `assertion` sections. Assertions are
exact ordered golden diagnostics, and the harness also checks batch/streaming
equivalence across chunk widths. Follow `src/diagnostics/TESTCASES.md` for the
schema and treat assertion changes as reviewed golden diffs.
