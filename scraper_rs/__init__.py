"""scraper_rs - Python bindings for the Rust scraper crate."""

from . import scraper_rs as _scraper_rs

__doc__ = _scraper_rs.__doc__
__all__ = list(
    getattr(
        _scraper_rs,
        "__all__",
        [name for name in dir(_scraper_rs) if not name.startswith("_")],
    )
)

globals().update({name: getattr(_scraper_rs, name) for name in __all__})
