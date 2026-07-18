# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-07-18

### Changed

- Renamed the project, packages, Rust crate imports, and CLI from
  `tokencompact` to `logcompact`.
- Moved the canonical repository to `ewhauser/logcompact`.

### Migration

- Replace `tokencompact-core` with `logcompact-core` and imports of
  `tokencompact_core` with `logcompact_core`.
- Replace `tokencompact-builtins` with `logcompact-builtins` and imports of
  `tokencompact_builtins` with `logcompact_builtins`.
- Replace `cargo install tokencompact` and the `tokencompact` executable with
  `cargo install logcompact` and `logcompact`.
- Existing `tokencompact` version `0.1.0` artifacts remain available for
  reproducible builds but are superseded and will not receive new releases.

## [0.1.0] - 2026-07-18

### Added

- Initial `tokencompact-core`, `tokencompact-builtins`, and `tokencompact`
  workspace.
