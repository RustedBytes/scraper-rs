# scraper-rs

Python bindings for the Rust `scraper` crate via PyO3. It gives you a lightweight `Document`/`Element` API with CSS selectors, handy helpers, and zero Python-side parsing work.

## Quick start

```py
from scraper_rs import Document, select, first

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

links_within_first = items[0].select("a[href]")
print([link.attr("href") for link in links_within_first])  # ["/a"]

first_link = doc.find("a[href]")
print(first_link.text, first_link.attr("href"))  # First / /a

# Functional helpers
links = select(html, "a[href]")
print([link.attr("href") for link in links])  # ["/a", "/b"]
print(first(html, "a[href]").text)            # First
```

For a runnable sample, see `examples/demo.py`.

## API highlights

- `Document(html: str)` / `Document.from_html(html)` parses once and keeps the DOM.
- `.select(css)` → `list[Element]`, `.find(css)` → first `Element | None`, `.css(css)` is an alias.
- `.text` returns normalized text; `.html` returns the original input.
- `Element` exposes `.tag`, `.text`, `.html`, `.attrs` plus helpers `.attr(name)`, `.get(name, default)`, `.to_dict()`.
- Elements support nested CSS selection via `.select(css)`, `.find(css)`, and `.css(css)`.
- Top-level helpers mirror the class methods: `parse(html)`, `select(html, css)`, `first(html, css)`.

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
