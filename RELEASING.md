# Releasing

All three packages use one workspace version and must be published in dependency
order:

1. `logcompact-core`
2. `logcompact-builtins`
3. `logcompact`

Release Please owns the shared version, changelog, `vX.Y.Z` tag, and GitHub
release. On every push to `main`, it creates or updates one release pull
request from Conventional Commits. Merging that pull request creates the tag
and release, then directly invokes the `Publish crates` and `Release assets`
workflows.

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
5. The same workflow publishes the crates and attaches native CLI archives for
   Linux x86-64 and ARM64, macOS Intel and Apple Silicon, and Windows x86-64.

Release Please only opens or updates a release pull request after a releasable
Conventional Commit such as `feat:`, `fix:`, or `perf:` reaches `main`. A
documentation, security, or maintenance-only `chore:` commit does not create an
empty release pull request.

## Release asset recovery

The `Release assets` workflow accepts an existing `vX.Y.Z` tag through manual
dispatch. It checks out that immutable tag, rebuilds all native archives,
attests them, and replaces matching assets on the existing GitHub release. Use
this path to resume a partial upload or backfill a release created before asset
automation was enabled.

The release configuration consistency check runs in `make check` and CI so a
new workspace crate or version location cannot silently fall out of the release
update.

## Trusted publishing

For each of the three crates on crates.io, add two trusted publishers with the
same repository and environment:

- repository owner: `ewhauser`
- repository: `logcompact`
- environment: `crates-io`
- workflow: `release-please.yml` for automated releases
- workflow: `publish.yml` for manual recovery

Both workflow entries are required. Although `release-please.yml` calls the
reusable `publish.yml` workflow, GitHub identifies the OIDC request to crates.io
by the calling workflow (`release-please.yml`). A manually dispatched recovery
run is identified by `publish.yml` instead.

Automated and manual workflow runs obtain a short-lived crates.io credential
through GitHub OIDC. Publication fails closed if trusted publishing is not
available; the repository and `crates-io` environment must not store a crates.io
API token.

For recovery, `Publish crates` can be dispatched manually with an existing
release tag. It checks out that tag, verifies the version and packages, and
skips any crate version that is already present on crates.io.

The publisher is resumable. If a run stops after publishing only part of the
workspace, rerun it from the same tag; already-published package versions are
verified against crates.io and skipped.

If trusted publishing is temporarily unavailable, repair the crates.io trusted
publisher record and dispatch `Publish crates` for the exact existing release
tag. Do not bypass the OIDC release identity with a long-lived API token.
