# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.12.0] - 2026-09-28

### Added

- Added version-specific PyPy 3.11 and PyPy 3.12 wheels for Linux x86-64,
  Linux ARM64, Windows, and macOS.
- Added PyPy 3.11 and 3.12 to the test matrix.
- Added release-time wheel-tag validation and installed-wheel smoke tests for
  every PyPy build.

### Changed

- Kept CPython 3.10+ wheels on `abi3` while making the CPython-only feature
  optional for version-specific PyPy builds.
- Pinned the upstream PyO3 FFI compatibility patch required by PyPy 3.12 until
  it is available in a published PyO3 release.
- Updated package metadata and build documentation to advertise PyPy support.

[0.12.0]: https://github.com/RustedBytes/scraper-rs/compare/v0.11.0...v0.12.0
