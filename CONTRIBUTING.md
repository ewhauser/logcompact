# Contributing

Contributions are welcome through GitHub issues and pull requests.

## Development setup

Install the Rust toolchain declared in `rust-toolchain.toml`, then run:

```console
make check
make fuzz-smoke
make package-check
```

`make check` verifies formatting, dependency boundaries, tests, Clippy, and
documentation. Fuzzing requires `cargo-fuzz` and a nightly Rust toolchain.

## Design rules

- Reducer behavior must be deterministic and independent of chunk boundaries.
- New parsers need focused unit tests and streaming-equivalence coverage.
- Parser order is an arbitration contract. Explain ordering changes in the PR.
- Never put secrets or unredacted raw log text in telemetry, snapshots, or
  test fixtures.
- Keep provider- and build-system-specific interpretation in downstream
  adapters, not these crates.
- Avoid new runtime dependencies. Explain any addition and its boundary impact.

Use Conventional Commit subjects. Release automation owns published versions;
ordinary feature pull requests should not edit versions.
