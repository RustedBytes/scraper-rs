# scraper-rs

[![PyPI - Version](https://img.shields.io/pypi/v/scraper-rust)](https://pypi.org/project/scraper-rust/)
[![Tests](https://github.com/RustedBytes/scraper-rs/actions/workflows/tests.yml/badge.svg)](https://github.com/RustedBytes/scraper-rs/actions/workflows/tests.yml)
[![PyPI Downloads](https://static.pepy.tech/personalized-badge/scraper-rust?period=total&units=INTERNATIONAL_SYSTEM&left_color=BLACK&right_color=GREEN&left_text=downloads)](https://pepy.tech/projects/scraper-rust)

Python bindings for the Rust [**scraper**](https://github.com/rust-scraper/scraper) crate via PyO3. It provides a lightweight `Document`/`Element` API with CSS selectors, XPath (via `xee-xpath`), handy helpers, and zero Python-side parsing work.

## Quick start

```py
from scraper_rs import Document, first, prettify, select, select_first, xpath

html = """
<html><body>
  <div class="item" data-id="1"><a href="/a">First</a></div>
  <div class="item" data-id="2"><a href="/b">Second</a></div>
</body></html>
"""

doc = Document(html)
print(doc.text)  # "First Second"

items = doc.select(".item")
print(items[0].attr("data-id"))  # "1"
print(items[0].to_dict())        # {"tag": "div", "text": "First", "html": "<a...>", ...}

first_link = doc.select_first("a[href]")  # alias: doc.find(...)
print(first_link.text, first_link.attr("href"))  # First /a
links_within_first = first_link.select("a[href]")
print([link.attr("href") for link in links_within_first])  # ["/a"]

# XPath (element results only)
xpath_items = doc.xpath("//div[@class='item']/a")
print([link.text for link in xpath_items])  # ["First", "Second"]
print(doc.xpath_first("//div[@data-id='1']/a").attr("href"))  # "/a"

# Functional helpers
links = select(html, "a[href]")
print([link.attr("href") for link in links])  # ["/a", "/b"]
print(first(html, "a[href]").text)            # First
print(select_first(html, "a[href]").text)     # First
print([link.text for link in xpath(html, "//div[@class='item']/a")])  # ["First", "Second"]
print(prettify(html))
```

For runnable samples, see `examples/demo.py` and `examples/demo_prettify_url.py`.
Quick URL prettify demo:

```sh
python examples/demo_prettify_url.py https://example.com --max-lines 80
```

### Async usage

The `scraper_rs.asyncio` module keeps the event loop responsive. `parse` yields once (`asyncio.sleep(0)`) before constructing a sync `Document` in the current thread, while `select`/`xpath` helpers run in a thread pool. Parsed documents and elements are wrapped with awaitable selector methods for nested queries:

```py
import asyncio
from scraper_rs import asyncio as scraping_async

html = "<div class='item'><a href='/a'>First</a></div>"


async def main():
    async with await scraping_async.parse(html) as doc:
        items = await doc.select(".item")
        first_link = await items[0].select_first("a[href]")
        print(first_link.text)  # First

    links = await scraping_async.select(html, "a[href]")
    print([link.attr("href") for link in links])  # ["/a"]


asyncio.run(main())
```

All async functions accept the same keyword arguments as their sync counterparts (`max_size_bytes`, `truncate_on_limit`, etc.).
Async wrappers expose the underlying sync objects via `.document` and `.element` if you need direct access.
`AsyncDocument` supports `async with` for automatic cleanup in coroutine code.

### Large documents and memory safety

To avoid runaway allocations, parsing defaults to a 1 GiB cap. Pass `max_size_bytes` to override:

```py
from scraper_rs import Document, select

doc = Document(html, max_size_bytes=5_000_000)  # 5 MB guard
links = select(html, "a[href]", max_size_bytes=5_000_000)
```

If you want to parse a limited portion of an oversized HTML document instead of rejecting it entirely, use `truncate_on_limit=True`:

```py
# Parse only the first 100KB of a large HTML document
doc = Document(large_html, max_size_bytes=100_000, truncate_on_limit=True)
links = doc.select("a[href]")  # Will only find links in the first 100KB

# Also works with top-level functions
items = select(large_html, ".item", max_size_bytes=100_000, truncate_on_limit=True)
```

Note: Truncation happens at valid UTF-8 character boundaries to prevent encoding errors.

## API highlights

- `Document(html: str)` / `Document.from_html(html)` parse HTML for CSS and keep the DOM; XPath parsing is initialized lazily on first XPath query.
- `.select(css)` → `list[Element]`, `.select_first(css)` / `.find(css)` → first `Element | None`, `.css(css)` is an alias.
- `.xpath(expr)` / `.xpath_first(expr)` evaluate XPath expressions that return element nodes.
- `.prettify()` renders the current DOM as an indented string for readable output/debugging.
- `.text` returns normalized text; `Document.html` is the original input HTML; `Element.html` is inner HTML.
- `scraper_rs.asyncio` exposes async `parse`/`select`/`xpath` wrappers to keep the event loop responsive.
- `Element` exposes `.tag`, `.text`, `.html`, `.attrs` plus helpers `.attr(name)`, `.get(name, default)`, `.to_dict()`.
- Elements support nested CSS and XPath selection via `.select(css)`, `.select_first(css)`, `.find(css)`, `.css(css)`, `.xpath(expr)`, `.xpath_first(expr)`.
- Elements also expose `.prettify()` to format element HTML with indentation.
- Top-level helpers mirror the class methods: `parse(html)`, `prettify(html)`, `select(html, css)`, `select_first(html, css)` / `first(html, css)`, `xpath(html, expr)`, `xpath_first(html, expr)`.
- `max_size_bytes` lets you fail fast on oversized HTML; defaults to a 1 GiB limit.
- `truncate_on_limit` allows parsing a truncated version (limited to `max_size_bytes`) of oversized HTML instead of raising an error.
- Call `doc.close()` (or `with Document(html) as doc: ...`) to free parsed DOM resources when you're done.
- In async workflows, use `async with await scraper_rs.asyncio.parse(html) as doc: ...` for automatic `AsyncDocument` cleanup.

## Installation

Built wheels target `abi3` (CPython 3.10+). To build locally:

```sh
# Install maturin (uv is used in this repo, but pip works too)
pip install maturin

# Build a wheel
maturin build --release --compatibility linux

# Install the generated wheel
pip install target/wheels/scraper_rust-*.whl
```

If you have `just` installed, the repo includes helpers: `just build` (local wheel), `just install-wheel` (install the built wheel), and `just build_manylinux` (via the official maturin Docker image).

## Projects Using scraper-rs

- [**silkworm**](https://github.com/BitingSnakes/silkworm) - Async web scraping framework on top of Rust

## Development

Requirements: Rust toolchain, Python 3.10+, `maturin`, `pytest`, and `pytest-asyncio` for tests.

- Run tests: `just test` or `uv run pytest tests/`
- Format code: `just fmt` (or `cargo fmt --all` and `uv run ruff format`)
- Lint Rust: `just lint` (or `cargo clippy --all-targets --all-features -- -D warnings`)
- The PyO3 module name is `scraper_rs`; the Rust crate is built as `cdylib`.

Contributions and issues are welcome. If you add public API, please extend `tests/test_scraper.py` and the example script accordingly.
