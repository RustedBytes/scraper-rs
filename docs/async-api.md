# Async API (scraper_rs.asyncio)

The async API lives in `scraper_rs/asyncio.py` and provides awaitable wrappers around the Rust extension module. Type stubs are in `scraper_rs/asyncio.pyi`.

## Key types

- `AsyncDocument`: wraps a synchronous `Document` and exposes awaitable selectors.
- `AsyncElement`: wraps a synchronous `Element` and exposes awaitable selectors.
- `AsyncDocument.document` and `AsyncElement.element` provide access to the underlying sync objects.

Implementation references:
- Python wrappers: `scraper_rs/asyncio.py`
- Rust async helpers: `src/lib.rs` (`select_async`, `select_first_async`, `xpath_async`, `xpath_first_async`)

## Top-level async functions

All async functions accept the same keyword arguments as the sync API (`max_size_bytes`, `truncate_on_limit`).

- `parse(html, **kwargs) -> AsyncDocument`
- `select(html, css, **kwargs) -> list[AsyncElement]`
- `select_first(html, css, **kwargs) -> AsyncElement | None`
- `first(html, css, **kwargs) -> AsyncElement | None`
- `xpath(html, expr, **kwargs) -> list[AsyncElement]`
- `xpath_first(html, expr, **kwargs) -> AsyncElement | None`

Example (from `examples/demo_asyncio.py`):

```py
import asyncio
from scraper_rs import asyncio as async_scraper

html = "<div class='item'><a href='/a'>First</a></div>"

async def main():
    doc = await async_scraper.parse(html)
    items = await doc.select(".item")
    first_link = await items[0].select_first("a[href]")
    print(first_link.text, first_link.attr("href"))

asyncio.run(main())
```

## How async execution works

- `scraper_rs/asyncio.py` calls Rust helpers such as `select_async` and `xpath_async`.
- In `src/lib.rs`, each async helper uses `pyo3_async_runtimes::tokio::future_into_py_with_locals` and `tokio::task::spawn_blocking` to run parsing and selection on a thread pool.
- `parse` is implemented in Python: it yields to the event loop with `asyncio.sleep(0)` and then constructs a sync `Document` in the current thread.

## Nested selection on AsyncElement

Nested selectors on `AsyncElement` call fragment helpers defined in `src/lib.rs`:

- `_select_fragment_async`
- `_select_first_fragment_async`
- `_xpath_fragment_async`
- `_xpath_first_fragment_async`

These helpers parse the element's inner HTML fragment each time (see `scraper_rs/asyncio.py` and `src/lib.rs`). This keeps the API simple but means repeated async queries re-parse the fragment.

## Performance notes

- Async selectors operate on HTML strings and parse on each call, which is ideal for one-shot async usage but can be less efficient for repeated queries over the same DOM.
- If you need repeated queries over a single document and can afford synchronous calls, use the sync `Document` directly (see `api.md`).
- For async workloads, batch operations with `asyncio.gather` to overlap parsing (see `tests/test_asyncio.py` and `examples/demo_asyncio.py`).
