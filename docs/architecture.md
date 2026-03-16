# Architecture

## Overview

`scraper_rs` is a Python extension module implemented in Rust with PyO3. The Rust code in `src/lib.rs` defines the public API (`Document`, `Element`, and top-level helper functions) and is exported as the Python module `scraper_rs`. The Python package in `scraper_rs/__init__.py` simply re-exports the extension module.

Async support lives in pure Python in `scraper_rs/asyncio.py`. It exposes `AsyncDocument` and `AsyncElement` wrappers to make selectors awaitable without storing a sync `Document` object in coroutine-facing state.

## Data flow (sync)

`Document` path:

1. Python calls `Document(...)` or `Document.from_html(...)`.
2. `Document::parse_with_limit` enforces size limits via `ensure_within_size_limit` in `src/lib.rs` (with optional truncation).
3. Construction parses HTML once into `scraper::Html` for CSS selectors and stores the raw HTML string.
4. CSS selection uses `parse_selector` and `Html::select`; parsed selectors are reused from a thread-local fixed-size cache (`SELECTOR_CACHE`).
5. XPath parsing is lazy: the first `xpath(...)` / `xpath_first(...)` call runs `ensure_xpath_state`, which parses `raw_html` into a `xee_xpath::Documents` store and keeps it for reuse.
6. XPath expressions are compiled via `compile_xpath` (to `SequenceQuery`) and reused from a thread-local fixed-size cache (`XPATH_CACHE`).
7. Selection results are converted into owned `Element` snapshots by `snapshot_element` (CSS path) and XPath sequence conversion helpers. `Element` computes `text`, inner `html`, and `attrs` lazily from stored element HTML on first access.

Top-level helper path:

- Functions like `select(...)` and `xpath(...)` call `*_with_limit` helpers and parse input per call (they do not reuse a persistent `Document` instance).

Key code references:
- Parsing and size limits: `src/lib.rs` (`DEFAULT_MAX_PARSE_BYTES`, `ensure_within_size_limit`, `Document::parse_with_limit`)
- CSS selection and selector cache: `src/lib.rs` (`SELECTOR_CACHE`, `parse_selector`, `snapshot_element`)
- Lazy XPath setup and XPath cache: `src/lib.rs` (`ensure_xpath_state`, `XPATH_CACHE`, `compile_xpath`)
- XPath selection and conversion: `src/lib.rs` (`execute_xpath_sequence`, `evaluate_xpath_sequence_elements`)

## Data flow (async)

The async layer is a thin wrapper over the sync API:

- The Python module `scraper_rs/asyncio.py` exposes `AsyncDocument` and `AsyncElement`.
- `AsyncDocument` stores only shareable `html` and `text` state; it does not retain a sync `Document`.
- Top-level async functions (`select`, `xpath`, `select_first`, `first`, etc) yield once and then call the corresponding synchronous Rust helper from `src/lib.rs`.
- `parse` yields once, constructs a temporary sync `Document`, copies out `html` and `text`, then drops the sync object.
- `AsyncElement` wraps an immutable sync `Element` snapshot and forwards nested selectors/prettify through awaitable methods.
- Selector calls still parse per call rather than reusing a persistent DOM, which keeps async objects thread-safe and lightweight.

Code references:
- Async wrappers: `scraper_rs/asyncio.py`
- Rust sync entry points: `src/lib.rs` (`Document`, `Element`, `select`, `select_first`, `first`, `xpath`, `xpath_first`)

## Module wiring

The Rust module initializer in `src/lib.rs` registers all classes and functions with the Python module:

- `#[pymodule] fn scraper_rs(...)` adds `Document`, `Element`, and the top-level functions.
- The version string is exposed as `__version__` from `env!("CARGO_PKG_VERSION")`.
- `scraper_rs/__init__.py` re-exports the supported sync API explicitly.

## Types and public surface

The runtime API is implemented in Rust, while type stubs live in:

- `scraper_rs.pyi` for the synchronous API.
- `scraper_rs/asyncio.pyi` for the asyncio wrappers.

The examples and tests are the best source of behavior expectations:

- Sync examples: `examples/demo.py`
- Async examples: `examples/demo_asyncio.py`
- Sync tests: `tests/test_scraper.py`
- Async tests: `tests/test_asyncio.py`
