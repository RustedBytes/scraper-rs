# scraper-rs documentation

This folder contains detailed documentation for the scraper-rs project (Python bindings for the Rust `scraper` crate). For a quick usage overview, see `README.md` at the repository root.

## Navigation

- `architecture.md` - how the Rust and Python layers fit together, data flow, and selector internals.
- `api.md` - synchronous API reference for `scraper_rs` with examples.
- `async-api.md` - asyncio wrappers, behavior, and usage patterns.
- `limits-and-errors.md` - size limits, truncation, and error behavior.
- `development.md` - build, test, and release workflows.

## Code map

- Rust extension module and core logic: `src/lib.rs`
- Python package export shim: `scraper_rs/__init__.py`
- Async wrappers: `scraper_rs/asyncio.py`
- Type stubs: `scraper_rs.pyi`, `scraper_rs/asyncio.pyi`
- Examples: `examples/demo.py`, `examples/demo_asyncio.py`
- Tests: `tests/test_scraper.py`, `tests/test_asyncio.py`
