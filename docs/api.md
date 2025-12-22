# Synchronous API (scraper_rs)

This document describes the public synchronous Python API implemented in `src/lib.rs` and typed in `scraper_rs.pyi`.

## Document

Constructor:

```py
from scraper_rs import Document

doc = Document(
    html,
    max_size_bytes=None,      # default: 1 GiB
    truncate_on_limit=False,  # default: error on oversized HTML
)
```

Key properties and methods (see `src/lib.rs` and `scraper_rs.pyi`):

- `html`: the original HTML string stored by the `Document`.
- `text`: normalized text content (whitespace collapsed).
- `select(css) -> list[Element]`: CSS selection over the whole document.
- `select_first(css) -> Element | None`: first CSS match.
- `find(css) -> Element | None`: alias for `select_first`.
- `css(css) -> list[Element]`: alias for `select`.
- `xpath(expr) -> list[Element]`: XPath selection (elements only).
- `xpath_first(expr) -> Element | None`: first XPath match.
- `close()`: free parsed DOMs and clear stored HTML.
- Context manager support: `with Document(html) as doc: ...`.

Implementation references:
- `Document::parse_with_limit` and `Document::new` in `src/lib.rs`
- `Document::close`, `__enter__`, `__exit__` in `src/lib.rs`

Example:

```py
from scraper_rs import Document

html = """
<html><body>
  <div class="item" data-id="1"><a href="/a">First</a></div>
  <div class="item" data-id="2"><a href="/b">Second</a></div>
</body></html>
"""

with Document(html) as doc:
    print(doc.text)  # "First Second"
    first_link = doc.find("a[href]")
    if first_link:
        print(first_link.text, first_link.attr("href"))
```

## Element

`Element` is a snapshot of a matched HTML element. It stores owned data (tag, text, inner HTML, and attributes) so it is safe to use after selection (see `snapshot_element` and `snapshot_xpath_element` in `src/lib.rs`).

Fields and methods:

- `tag`, `text`, `html`, `attrs`
- `attr(name) -> str | None`: return a single attribute value.
- `get(name, default=None) -> str | None`: dict-style access with default.
- `to_dict() -> dict`: serialize the element fields.
- Selector helpers: `select`, `select_first`, `find`, `css`, `xpath`, `xpath_first`.

Element selection uses fragment parsing helpers in `src/lib.rs`:
- `select_fragment` for CSS, `evaluate_fragment_xpath` for XPath.

Example (nested selection):

```py
item = doc.find(".item")
if item:
    link = item.select_first("a[href]")
    if link:
        print(link.text, link.attr("href"))
```

## Top-level helper functions

The top-level helpers parse the HTML and immediately run the query. They are useful for one-shot usage:

- `parse(html, *, max_size_bytes=None, truncate_on_limit=False) -> Document`
- `select(html, css, *, max_size_bytes=None, truncate_on_limit=False) -> list[Element]`
- `select_first(html, css, *, max_size_bytes=None, truncate_on_limit=False) -> Element | None`
- `first(html, css, *, max_size_bytes=None, truncate_on_limit=False) -> Element | None`
- `xpath(html, expr, *, max_size_bytes=None, truncate_on_limit=False) -> list[Element]`
- `xpath_first(html, expr, *, max_size_bytes=None, truncate_on_limit=False) -> Element | None`

These functions are defined in `src/lib.rs` and registered in the module initializer at the bottom of the file.

Example:

```py
from scraper_rs import select, xpath_first

links = select(html, "a[href]")
first = xpath_first(html, "//div[@class='item']/a")
```

## Version and type hints

- `__version__` is exported from `src/lib.rs` using the Cargo package version.
- `scraper_rs.pyi` provides type hints for all public classes and functions.

## Behavior notes

- `Element.html` is the inner HTML (children only), not the outer tag.
- `text` values are normalized by collapsing whitespace.
- XPath expressions must return element nodes; attribute or text results raise `ValueError`.
- Invalid CSS or XPath expressions raise `ValueError` from the Rust layer.

See `limits-and-errors.md` for details about size limits and error messages.
