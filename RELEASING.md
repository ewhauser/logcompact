# Releasing

All three packages use one workspace version and must be published in dependency
order:

1. `tokencompact-core`
2. `tokencompact-builtins`
3. `tokencompact`

The `Publish crates` GitHub Actions workflow performs this sequence and waits
for each dependency to become visible before publishing its consumer.

## First release bootstrap

crates.io trusted publishing can only be configured after a crate exists. For
the first release:

1. Add an environment named `crates-io` to the GitHub repository.
2. Add a short-lived crates.io API token as the environment secret
   `CARGO_REGISTRY_TOKEN`.
3. Run the `Publish crates` workflow from the version tag or commit to publish.
4. Remove the token after the workflow completes.

## Trusted publishing

For each of the three crates on crates.io, add a trusted publisher with:

- repository owner: `ewhauser`
- repository: `tokencompact`
- workflow: `publish.yml`
- environment: `crates-io`

Then set the GitHub repository variable `CRATES_IO_TRUSTED_PUBLISHING` to
`true`. Future manual workflow runs obtain a short-lived crates.io credential
through GitHub OIDC and do not require a stored API token.

Before publishing, update the shared workspace version, update the changelog,
run `make check`, `make fuzz-smoke`, and `make package-check`, commit, and tag
the exact release commit as `vX.Y.Z`.
