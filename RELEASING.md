# Releasing

All three packages use one workspace version and must be published in dependency
order:

1. `logcompact-core`
2. `logcompact-builtins`
3. `logcompact`

The `Publish crates` GitHub Actions workflow performs this sequence and waits
for each dependency to become visible before publishing its consumer.

## New-name release bootstrap

crates.io trusted publishing can only be configured after a crate exists. For
the first `logcompact` release:

1. Add an environment named `crates-io` to the GitHub repository.
2. Add a short-lived crates.io API token as the environment secret
   `CARGO_REGISTRY_TOKEN`.
3. Run the `Publish crates` workflow from the exact `vX.Y.Z` version tag.
4. Remove the token after the workflow completes.

The superseded `tokencompact*` packages are intentionally left at 0.1.0 and
must not be included in future release tags or publication runs.

The publisher is resumable. If a run stops after publishing only part of the
workspace, rerun it from the same tag; already-published package versions are
verified against crates.io and skipped.

## Trusted publishing

For each of the three crates on crates.io, add a trusted publisher with:

- repository owner: `ewhauser`
- repository: `logcompact`
- workflow: `publish.yml`
- environment: `crates-io`

Then set the GitHub repository variable `CRATES_IO_TRUSTED_PUBLISHING` to
`true`. Future manual workflow runs obtain a short-lived crates.io credential
through GitHub OIDC and do not require a stored API token.

Before publishing, update the shared workspace version, update the changelog,
run `make check`, `make fuzz-smoke`, and `make package-check`, commit, and tag
the exact release commit as `vX.Y.Z`.
