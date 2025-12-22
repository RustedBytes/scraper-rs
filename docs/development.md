# Development workflow

This document summarizes the build, test, and release workflow with references to the project configuration files.

## Project layout

- Rust extension module: `src/lib.rs`
- Python package shim and async wrappers: `scraper_rs/__init__.py`, `scraper_rs/asyncio.py`
- Type stubs: `scraper_rs.pyi`, `scraper_rs/asyncio.pyi`
- Examples: `examples/demo.py`, `examples/demo_asyncio.py`
- Tests: `tests/test_scraper.py`, `tests/test_asyncio.py`
- Build metadata: `Cargo.toml`, `pyproject.toml`

## Common tasks (justfile)

The `justfile` in the repo root defines the standard workflow:

```sh
just init           # create a uv virtual env
just install        # install dev dependencies
just build          # build a release wheel
just install-wheel  # install the built wheel
just test           # run pytest
just fmt            # run cargo fmt and ruff format
just lint           # run cargo clippy
just clean          # remove build artifacts and caches
```

See `justfile` for the exact commands and flags.

## Building manually

The Rust extension is built with `maturin` (configured in `pyproject.toml`). The simplest manual build is:

```sh
maturin build --release --compatibility linux
pip install target/wheels/scraper_rust-*.whl
```

## Testing

- Sync tests: `tests/test_scraper.py`
- Async tests: `tests/test_asyncio.py`

Run them with:

```sh
pytest tests/
```

or via `just test`.

## Release and CI

Versioning is defined in both `Cargo.toml` and `pyproject.toml` and should stay in sync. CI and release automation live in:

- `.github/workflows/tests.yml`
- `.github/workflows/release.yml`
- `.github/workflows/bump-version.yml`

The release workflow builds manylinux wheels using the official maturin Docker image (see `justfile` for the local analog).
