# Architecture

## Overview

`scraper_rs` is a Python extension module implemented in Rust with PyO3. The Rust code in `src/lib.rs` defines the public API (`Document`, `Element`, and top-level helper functions) and is exported as the Python module `scraper_rs`. The Python package in `scraper_rs/__init__.py` simply re-exports the extension module.

Async support lives in pure Python in `scraper_rs/asyncio.py`. It wraps the Rust async helpers defined in `src/lib.rs` and exposes `AsyncDocument` and `AsyncElement` wrappers to make selectors awaitable.

## Data flow (sync)

`Document` path:

1. Python calls `Document(...)` or `Document.from_html(...)`.
2. `Document::parse_with_limit` enforces size limits via `ensure_within_size_limit` in `src/lib.rs` (with optional truncation).
3. Construction parses HTML once into `scraper::Html` for CSS selectors and stores the raw HTML string.
4. CSS selection uses `parse_selector` and `Html::select`; parsed selectors are reused from a thread-local fixed-size cache (`SELECTOR_CACHE`).
5. XPath parsing is lazy: the first `xpath(...)` / `xpath_first(...)` call runs `ensure_xpath_package`, which parses `raw_html` with `sxd_html::parse_html` and stores it in `xpath_package` for reuse.
6. XPath expressions are compiled via `compile_xpath` and reused from a thread-local fixed-size cache (`XPATH_CACHE`).
7. Selection results are converted into owned `Element` snapshots by `snapshot_element` / `snapshot_xpath_element`. `Element` computes `text`, inner `html`, and `attrs` lazily from stored element HTML on first access.

Top-level helper path:

- Functions like `select(...)` and `xpath(...)` call `*_with_limit` helpers and parse input per call (they do not reuse a persistent `Document` instance).

Key code references:
- Parsing and size limits: `src/lib.rs` (`DEFAULT_MAX_PARSE_BYTES`, `ensure_within_size_limit`, `Document::parse_with_limit`)
- CSS selection and selector cache: `src/lib.rs` (`SELECTOR_CACHE`, `parse_selector`, `snapshot_element`)
- Lazy XPath setup and XPath cache: `src/lib.rs` (`ensure_xpath_package`, `XPATH_CACHE`, `compile_xpath`)
- XPath selection and serialization: `src/lib.rs` (`evaluate_xpath_nodes`, `snapshot_xpath_element`, `serialize_node_into`)

## Data flow (async)

The async layer is a thin wrapper over the sync API:

- The Python module `scraper_rs/asyncio.py` exposes `AsyncDocument` and `AsyncElement`.
- Top-level async functions (`select`, `xpath`, `select_first`, `first`, etc) call Rust helpers in `src/lib.rs` such as `select_async`, `first_async`, and `xpath_async`.
- The Rust async helpers use `pyo3_async_runtimes::tokio::future_into_py_with_locals` and `tokio::task::spawn_blocking` to run blocking parsing and selection on a thread pool.
- `parse` is implemented in Python (`asyncio.sleep(0)` + sync `Document(...)` construction).
- `AsyncDocument` / `AsyncElement` selector methods pass HTML strings into Rust async helpers, so selectors parse per call rather than reusing the wrapped `Document` DOM.
- Nested async selection on elements is implemented via `_select_fragment_async` and `_xpath_fragment_async` in `src/lib.rs`, which parse the element's inner HTML as a fragment.

Code references:
- Async wrappers: `scraper_rs/asyncio.py`
- Rust async entry points: `src/lib.rs` (`select_async`, `select_first_async`, `first_async`, `xpath_async`, `xpath_first_async`)
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
