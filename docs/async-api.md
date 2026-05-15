# Async API (scraper_rs.asyncio)

The async API lives in `scraper_rs/asyncio.py` and provides awaitable wrappers around the Rust extension module. Type stubs are in `scraper_rs/asyncio.pyi`.

## Key types

- `AsyncDocument`: stores shareable document state (`html`, `text`) and exposes awaitable selectors.
- `AsyncElement`: wraps an immutable element snapshot and exposes awaitable nested selectors.

Implementation references:
- Python wrappers: `scraper_rs/asyncio.py`
- Sync primitives: `src/document.rs`, `src/element.rs`, and `src/functions.rs`

## Top-level async functions

All async functions accept the same keyword arguments as the sync API (`max_size_bytes`, `truncate_on_limit`).

- `parse(html, **kwargs) -> AsyncDocument`
- `select(html, css, **kwargs) -> list[AsyncElement]`
- `select_first(html, css, **kwargs) -> AsyncElement | None`
- `first(html, css, **kwargs) -> AsyncElement | None`
- `xpath(html, expr, **kwargs) -> list[AsyncElement]`
- `xpath_first(html, expr, **kwargs) -> AsyncElement | None`
- `prettify(html, **kwargs) -> str`

Example (from `examples/demo_asyncio.py`):

```py
import asyncio
from scraper_rs import asyncio as async_scraper

html = "<div class='item'><a href='/a'>First</a></div>"

async def main():
    async with await async_scraper.parse(html) as doc:
        items = await doc.select(".item")
        first_link = await items[0].select_first("a[href]")
        print(first_link.text, first_link.attr("href"))

asyncio.run(main())
```

## Context management and cleanup

- `AsyncDocument` supports `async with` and clears its stored HTML/text state on exit (including exception paths).
- `AsyncDocument` also supports sync `with`, but `async with` is preferred inside coroutine code.

```py
doc = await async_scraper.parse(html)
async with doc:
    items = await doc.select(".item")
```

## How async execution works

- `scraper_rs/asyncio.py` presents an async API even though the parsing primitives remain synchronous in Rust.
- Each async entry point yields to the event loop once before invoking the corresponding synchronous Rust helper.
- `AsyncDocument` stores only `html` and `text`, so coroutine code does not hold a thread-affine sync `Document`.
- Selector calls still parse per call, just like the sync top-level helper functions.

## Nested selection on AsyncElement

Nested selectors on `AsyncElement` call the wrapped sync `Element` snapshot methods from `src/element.rs`.
Those methods operate on the stored element HTML fragment each time, which keeps the async wrapper simple but means repeated nested queries re-parse the fragment.

## Performance notes

- Async selectors operate on HTML strings and parse on each call, which is ideal for one-shot async usage but can be less efficient for repeated queries over the same DOM.
- If you need repeated queries over a single document and can afford synchronous calls, use the sync `Document` directly (see `api.md`).
- `asyncio.gather` is useful for keeping calling code uniform, but these Python wrappers do not make CPU-bound parsing parallel on their own.
