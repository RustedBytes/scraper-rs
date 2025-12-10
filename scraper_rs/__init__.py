"""scraper_rs - Python bindings for the Rust scraper crate."""

# Import everything from the Rust extension module
from .scraper_rs import *

__doc__ = scraper_rs.__doc__
if hasattr(scraper_rs, "__all__"):
    __all__ = scraper_rs.__all__
