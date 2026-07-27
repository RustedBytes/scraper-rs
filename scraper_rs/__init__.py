"""scraper_rs - Python bindings for Rust HTML parsing and selection."""

from .. import scraper_rs as _scraper_rs

__doc__ = _scraper_rs.__doc__
__all__ = [
    "Document",
    "Element",
    "__version__",
    "first",
    "parse",
    "parse_document",
    "parse_fragment",
    "prettify",
    "select",
    "select_first",
    "xpath",
    "xpath_first",
]

globals().update({name: getattr(_scraper_rs, name) for name in __all__})
