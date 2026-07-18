# Releasing

All three packages use one workspace version and must be published in dependency
order:

1. `logcompact-core`
2. `logcompact-builtins`
3. `logcompact`

Release Please owns the shared version, changelog, `vX.Y.Z` tag, and GitHub
release. On every push to `main`, it creates or updates one release pull
request from Conventional Commits. Merging that pull request creates the tag
and release, then directly invokes the `Publish crates` workflow.

The publisher performs the dependency-ordered sequence above and waits for each
dependency to become visible before publishing its consumer. It is called
directly from the Release Please workflow because tags created with the default
GitHub Actions token do not start separate tag-triggered workflows.

## Release lifecycle

1. Merge Conventional Commits to `main`.
2. Review and merge the `chore(main): release X.Y.Z` pull request maintained by
   Release Please.
3. Release Please updates `CHANGELOG.md`, `version.txt`, the workspace package
   and dependency versions, and the matching `Cargo.lock` package versions.
4. Release Please creates `vX.Y.Z` and the GitHub release.
5. The same workflow calls `Publish crates` for that exact tag.

The release configuration consistency check runs in `make check` and CI so a
new workspace crate or version location cannot silently fall out of the release
update.

## Trusted publishing

For each of the three crates on crates.io, add a trusted publisher with:

- repository owner: `ewhauser`
- repository: `logcompact`
- workflow: `publish.yml`
- environment: `crates-io`

Then set the GitHub repository variable `CRATES_IO_TRUSTED_PUBLISHING` to
`true`. Automated and manual workflow runs obtain a short-lived crates.io
credential through GitHub OIDC and do not require a stored API token.

For recovery, `Publish crates` can be dispatched manually with an existing
release tag. It checks out that tag, verifies the version and packages, and
skips any crate version that is already present on crates.io.

The publisher is resumable. If a run stops after publishing only part of the
workspace, rerun it from the same tag; already-published package versions are
verified against crates.io and skipped.

If trusted publishing is temporarily unavailable, add a short-lived crates.io
API token as the `CARGO_REGISTRY_TOKEN` secret on the `crates-io` environment,
dispatch `Publish crates` for the exact release tag, and remove the token after
the run completes.
