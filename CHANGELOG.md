# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.16.0] - 2026-10-07

### Changed

- Improved XPath selector ranking with semantic attributes and ancestor anchors,
  normalized link/media paths, and lower priority for generated IDs and classes;
  added normalized-text fallback for attribute-less targets
  ([#64](https://github.com/RustedBytes/scraper-rs/pull/64)).
- Limited automatic tests to relevant changes in non-draft PRs using four Python
  interpreters, canceled superseded runs, and made benchmarks and the full test
  matrix available through manual runs
  ([#65](https://github.com/RustedBytes/scraper-rs/pull/65)).

### Fixed

- Filled missing changelog entries for versions 0.13.0 through 0.15.0
  ([#63](https://github.com/RustedBytes/scraper-rs/pull/63)).

## [0.15.0] - 2026-10-07

### Changed

- Optimized XPath selector generation by skipping searches proven unable to
  distinguish the target and returning the validated absolute positional
  fallback ([#62](https://github.com/RustedBytes/scraper-rs/pull/62)).

## [0.14.0] - 2026-10-06

### Changed

- Improved XPath attribute ranking for scraping by prioritizing `href` on
  links and `src` on content-bearing media, with tag-aware selection
  ([#61](https://github.com/RustedBytes/scraper-rs/pull/61)).

## [0.13.0] - 2026-10-06

### Added

- Added unique CSS selector generation and Robula+-style XPath selector
  generation for document elements, with an absolute positional XPath fallback
  ([#59](https://github.com/RustedBytes/scraper-rs/pull/59)).

### Fixed

- Fixed duplicate workflow keys in Tests and Benchmark, and replaced the
  nonexistent benchmark script with the existing Criterion targets
  ([#60](https://github.com/RustedBytes/scraper-rs/pull/60)).

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

[0.16.0]: https://github.com/RustedBytes/scraper-rs/compare/v0.15.0...v0.16.0
[0.15.0]: https://github.com/RustedBytes/scraper-rs/compare/v0.14.0...v0.15.0
[0.14.0]: https://github.com/RustedBytes/scraper-rs/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/RustedBytes/scraper-rs/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/RustedBytes/scraper-rs/compare/v0.11.0...v0.12.0
