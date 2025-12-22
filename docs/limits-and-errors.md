# Limits, truncation, and errors

This document describes size limits, truncation, and error behavior implemented in `src/lib.rs` and validated by `tests/test_scraper.py` and `tests/test_asyncio.py`.

## Size limits

By default, parsing is capped at 1 GiB (`DEFAULT_MAX_PARSE_BYTES` in `src/lib.rs`). The limit applies to:

- `Document(...)` and `Document.from_html(...)`
- Top-level helpers (`parse`, `select`, `select_first`, `first`, `xpath`, `xpath_first`)
- Async helpers (`scraper_rs.asyncio.*`)

You can override the limit with `max_size_bytes`:

```py
from scraper_rs import Document, select

# Allow up to 5 MB
limit = 5_000_000

doc = Document(html, max_size_bytes=limit)
links = select(html, "a[href]", max_size_bytes=limit)
```

If the HTML is larger than the limit, a `ValueError` is raised:

```
ValueError: HTML document is too large to parse: ...
```

This behavior is enforced by `ensure_within_size_limit` in `src/lib.rs`.

## Truncation

If you prefer parsing a truncated document instead of raising an error, set `truncate_on_limit=True`. Truncation is UTF-8 safe: the code backs up to a character boundary to avoid breaking multi-byte characters (see `ensure_within_size_limit` in `src/lib.rs` and `test_truncate_utf8_boundary` in `tests/test_scraper.py`).

```py
from scraper_rs import Document

# Parse only the first 100 KB
small_doc = Document(html, max_size_bytes=100_000, truncate_on_limit=True)
```

Async usage is identical:

```py
from scraper_rs import asyncio as async_scraper

items = await async_scraper.select(
    html, ".item", max_size_bytes=100_000, truncate_on_limit=True
)
```

## Selector errors

### CSS selector errors

Invalid CSS selectors raise `ValueError` from `parse_selector` in `src/lib.rs`.

```py
from scraper_rs import Document

try:
    Document(html).select("div[")
except ValueError as exc:
    print(exc)
```

### XPath errors

Invalid XPath expressions or expressions that do not return a node set raise `ValueError` from `evaluate_xpath_nodes` in `src/lib.rs`. XPath queries must return element nodes; attribute or text results are not supported (see `snapshot_xpath_element`).

```py
from scraper_rs import Document

try:
    # This returns attributes, not elements
    Document(html).xpath("//div/@class")
except ValueError as exc:
    print(exc)
```

## Document lifecycle

`Document.close()` explicitly releases DOM allocations and clears the stored HTML. After closing, selectors return no results and `html`/`text` are empty (validated in `tests/test_scraper.py`). `Document` also acts as a context manager:

```py
from scraper_rs import Document

with Document(html) as doc:
    assert doc.find("a") is not None
# After exiting, the document is cleared
```
