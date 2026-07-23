# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0](https://github.com/ewhauser/logcompact/compare/v0.3.3...v0.4.0) (2026-07-20)


### ⚠ BREAKING CHANGES

* **core:** integrate Hawk and remove dead APIs ([#41](https://github.com/ewhauser/logcompact/issues/41))

### Features

* **builtins:** add pytest diagnostic coverage ([#23](https://github.com/ewhauser/logcompact/issues/23)) ([18b4671](https://github.com/ewhauser/logcompact/commit/18b46712311dbb8bc872e05b94c9d3f488e30663))


### Bug Fixes

* require tag-capable Release Please ([#21](https://github.com/ewhauser/logcompact/issues/21)) ([b041d3d](https://github.com/ewhauser/logcompact/commit/b041d3d79aee367acbee2adb6e1ff6b49bb02880))


### Code Refactoring

* **core:** integrate Hawk and remove dead APIs ([#41](https://github.com/ewhauser/logcompact/issues/41)) ([230edd2](https://github.com/ewhauser/logcompact/commit/230edd22bc3e1ed0f8db20dcf6aa04b50556f658))

## [0.3.3](https://github.com/ewhauser/logcompact/compare/v0.3.2...v0.3.3) (2026-07-19)


### Bug Fixes

* attach native CLI release assets ([#16](https://github.com/ewhauser/logcompact/issues/16)) ([188e46d](https://github.com/ewhauser/logcompact/commit/188e46d80ade4d804f991b4666b0923a15158b3f))
* encode release artifact output ([#18](https://github.com/ewhauser/logcompact/issues/18)) ([44e0ca3](https://github.com/ewhauser/logcompact/commit/44e0ca3bcd33099d9ef094d06ca8bd8f660c859f))
* publish release after attaching assets ([#19](https://github.com/ewhauser/logcompact/issues/19)) ([005925c](https://github.com/ewhauser/logcompact/commit/005925c6c0c94d80c3b32806aeeec29d9b270d56))

## [0.3.2](https://github.com/ewhauser/logcompact/compare/v0.3.1...v0.3.2) (2026-07-19)


### Performance Improvements

* exceed 150 MB/s built-in parsing ([#14](https://github.com/ewhauser/logcompact/issues/14)) ([9a8b8da](https://github.com/ewhauser/logcompact/commit/9a8b8da30cfd4c2436160deee0c54cb731ef5cfc))

## [0.3.1](https://github.com/ewhauser/logcompact/compare/v0.3.0...v0.3.1) (2026-07-19)


### Bug Fixes

* harden release publishing configuration ([#10](https://github.com/ewhauser/logcompact/issues/10)) ([52d90ed](https://github.com/ewhauser/logcompact/commit/52d90ed5b18d2762d418a3868f745a48ed6b009c))


### Performance Improvements

* accelerate built-in diagnostic matching ([#12](https://github.com/ewhauser/logcompact/issues/12)) ([9c60d76](https://github.com/ewhauser/logcompact/commit/9c60d7608d1660550b4aa47e1292bec8ddb8084f))

## [0.3.0](https://github.com/ewhauser/logcompact/compare/v0.2.0...v0.3.0) (2026-07-18)


### Features

* add customizable problem matchers ([#6](https://github.com/ewhauser/logcompact/issues/6)) ([d579cda](https://github.com/ewhauser/logcompact/commit/d579cda75dc5a383769ec77d2794a1d53b94b31f))


### Performance Improvements

* establish reusable benchmark ownership ([#5](https://github.com/ewhauser/logcompact/issues/5)) ([3aba772](https://github.com/ewhauser/logcompact/commit/3aba772cf6a3ca696d01bbe8ac7292f08170e9f3))

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
