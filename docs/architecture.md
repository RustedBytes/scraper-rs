# Architecture

## Overview

`scraper_rs` is a Python extension module implemented in Rust with PyO3. The Rust code in `src/lib.rs` defines the public API (`Document`, `Element`, and top-level helper functions) and is exported as the Python module `scraper_rs`. The Python package in `scraper_rs/__init__.py` simply re-exports the extension module.

Async support lives in pure Python in `scraper_rs/asyncio.py`. It wraps the Rust async helpers defined in `src/lib.rs` and exposes `AsyncDocument` and `AsyncElement` wrappers to make selectors awaitable.

## Data flow (sync)

1. Python calls `Document(...)` or a top-level helper like `select(...)` from the extension module.
2. `Document::parse_with_limit` enforces size limits via `ensure_within_size_limit` in `src/lib.rs`.
3. The HTML is parsed twice:
   - `scraper::Html` for CSS selectors.
   - `sxd_document::Package` (via `sxd_html::parse_html`) for XPath selectors.
4. CSS selection uses `parse_selector` and `Html::select`, then converts results with `snapshot_element` into owned `Element` values.
5. XPath selection uses `sxd_xpath::Factory` and `Context` to evaluate the expression, then converts nodes with `snapshot_xpath_element`. XPath inner HTML is built by `serialize_children` and `serialize_node_into`.
6. `Element` stores owned strings and attribute maps, so it is safe to use on the Python side without Rust lifetimes.

Key code references:
- Parsing and size limits: `src/lib.rs` (`DEFAULT_MAX_PARSE_BYTES`, `ensure_within_size_limit`, `Document::parse_with_limit`)
- CSS selection: `src/lib.rs` (`parse_selector`, `select_fragment`, `snapshot_element`)
- XPath selection: `src/lib.rs` (`evaluate_xpath_nodes`, `evaluate_xpath_elements`, `snapshot_xpath_element`)

## Data flow (async)

The async layer is a thin wrapper over the sync API:

- The Python module `scraper_rs/asyncio.py` exposes `AsyncDocument` and `AsyncElement`.
- Top-level async functions (`select`, `xpath`, `select_first`, etc) call Rust helpers in `src/lib.rs` such as `select_async` and `xpath_async`.
- The Rust async helpers use `pyo3_async_runtimes::tokio::future_into_py_with_locals` and `tokio::task::spawn_blocking` to run blocking parsing and selection on a thread pool.
- Nested async selection on elements is implemented via `_select_fragment_async` and `_xpath_fragment_async` in `src/lib.rs`, which parse the element's inner HTML as a fragment.

Code references:
- Async wrappers: `scraper_rs/asyncio.py`
- Rust async entry points: `src/lib.rs` (`select_async`, `select_first_async`, `xpath_async`, `xpath_first_async`)
- Fragment helpers: `src/lib.rs` (`_select_fragment_async`, `_xpath_fragment_async`)

## Module wiring

The Rust module initializer in `src/lib.rs` registers all classes and functions with the Python module:

- `#[pymodule] fn scraper_rs(...)` adds `Document`, `Element`, and the top-level functions.
- The version string is exposed as `__version__` from `env!("CARGO_PKG_VERSION")`.
- `scraper_rs/__init__.py` re-exports the extension module and forwards `__all__` if it exists.

## Types and public surface

The runtime API is implemented in Rust, while type stubs live in:

- `scraper_rs.pyi` for the synchronous API.
- `scraper_rs/asyncio.pyi` for the asyncio wrappers.

The examples and tests are the best source of behavior expectations:

- Sync examples: `examples/demo.py`
- Async examples: `examples/demo_asyncio.py`
- Sync tests: `tests/test_scraper.py`
- Async tests: `tests/test_asyncio.py`
