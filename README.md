# scraper-rs

![PyPI - Version](https://img.shields.io/pypi/v/scraper-rust)

Python bindings for the Rust `scraper` crate via PyO3. It gives you a lightweight `Document`/`Element` API with CSS selectors, XPath (via `sxd_html`/`sxd_xpath`), handy helpers, and zero Python-side parsing work.

## Quick start

```py
from scraper_rs import Document, first, select, select_first, xpath

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
print(first_link.text, first_link.attr("href"))  # First / /a
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
```

For a runnable sample, see `examples/demo.py`.

### Large documents and memory safety

To avoid runaway allocations, parsing defaults to a 1 GiB cap. Pass `max_size_bytes` to override:

```py
from scraper_rs import Document, select

doc = Document(html, max_size_bytes=5_000_000)  # 5 MB guard
links = select(html, "a[href]", max_size_bytes=5_000_000)
```

## API highlights

- `Document(html: str)` / `Document.from_html(html)` parses once and keeps the DOM.
- `.select(css)` → `list[Element]`, `.select_first(css)` / `.find(css)` → first `Element | None`, `.css(css)` is an alias.
- `.xpath(expr)` / `.xpath_first(expr)` evaluate XPath expressions that return element nodes.
- `.text` returns normalized text; `.html` returns the original input.
- `Element` exposes `.tag`, `.text`, `.html`, `.attrs` plus helpers `.attr(name)`, `.get(name, default)`, `.to_dict()`.
- Elements support nested CSS and XPath selection via `.select(css)`, `.select_first(css)`, `.find(css)`, `.css(css)`, `.xpath(expr)`, `.xpath_first(expr)`.
- Top-level helpers mirror the class methods: `parse(html)`, `select(html, css)`, `select_first(html, css)` / `first(html, css)`, `xpath(html, expr)`, `xpath_first(html, expr)`.
- `max_size_bytes` lets you fail fast on oversized HTML; defaults to a 1 GiB limit.
- Call `doc.close()` (or `with Document(html) as doc: ...`) to free parsed DOM resources when you're done.

## Installation

Built wheels target `abi3` (CPython 3.10+). To build locally:

```sh
# Install maturin (uv is used in this repo, but pip works too)
pip install maturin

# Build a wheel
maturin build --release --compatibility linux

# Install the generated wheel
pip install target/wheels/scraper_rs-*.whl
```

If you have `just` installed, the repo includes helpers: `just build` (local wheel), `just install-wheel` (install the built wheel), and `just build_manylinux` (via the official maturin Docker image).

## Development

Requirements: Rust toolchain, Python 3.10+, `maturin`, and `pytest` for tests.

- Run tests: `just test` or `uv run pytest tests/test_scraper.py`
- Format/typing: Rust and Python are small; no formatters are enforced yet.
- The PyO3 module name is `scraper_rs`; the Rust crate is built as `cdylib`.

Contributions and issues are welcome. If you add public API, please extend `tests/test_scraper.py` and the example script accordingly.
