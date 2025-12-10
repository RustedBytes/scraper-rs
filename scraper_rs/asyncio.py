"""Asyncio wrappers for scraper_rs functions.

This module provides async versions of the main scraper_rs functions,
allowing them to be used in asyncio applications without blocking the event loop.

Note: The Document class cannot be passed between threads due to PyO3 limitations,
so parse() executes in the current thread but yields control to the event loop.
The select() and xpath() functions can run in a thread pool since they return
only Element objects which are thread-safe.
"""

import asyncio
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from . import Document, Element

# Import the synchronous functions from the main module
# These will be available after the package is built
from . import Document as _Document
from . import select as _select
from . import select_first as _select_first
from . import xpath as _xpath
from . import xpath_first as _xpath_first


async def parse(html: str, **kwargs) -> "Document":
    """Parse HTML asynchronously.

    Note: Due to PyO3 limitations, the Document is created in the current thread
    but yields control to the event loop to avoid blocking.

    Args:
        html: The HTML string to parse
        **kwargs: Additional arguments (max_size_bytes, truncate_on_limit, etc.)

    Returns:
        A Document object
    """
    # Yield control to the event loop before parsing
    await asyncio.sleep(0)
    # Parse in the current thread (Document is unsendable)
    return _Document(html, **kwargs)


async def select(html: str, css: str, **kwargs) -> list["Element"]:
    """Select elements by CSS selector asynchronously.

    This function runs in a thread pool to avoid blocking the event loop.

    Args:
        html: The HTML string to parse
        css: CSS selector string
        **kwargs: Additional arguments (max_size_bytes, truncate_on_limit, etc.)

    Returns:
        A list of Element objects matching the CSS selector
    """
    return await asyncio.to_thread(_select, html, css, **kwargs)


async def select_first(html: str, css: str, **kwargs) -> "Element | None":
    """Select the first element by CSS selector asynchronously.

    This function runs in a thread pool to avoid blocking the event loop.

    Args:
        html: The HTML string to parse
        css: CSS selector string
        **kwargs: Additional arguments (max_size_bytes, truncate_on_limit, etc.)

    Returns:
        The first Element matching the CSS selector, or None if no match
    """
    return await asyncio.to_thread(_select_first, html, css, **kwargs)


async def xpath(html: str, expr: str, **kwargs) -> list["Element"]:
    """Select elements by XPath expression asynchronously.

    This function runs in a thread pool to avoid blocking the event loop.

    Args:
        html: The HTML string to parse
        expr: XPath expression string
        **kwargs: Additional arguments (max_size_bytes, truncate_on_limit, etc.)

    Returns:
        A list of Element objects matching the XPath expression
    """
    return await asyncio.to_thread(_xpath, html, expr, **kwargs)


async def xpath_first(html: str, expr: str, **kwargs) -> "Element | None":
    """Select the first element by XPath expression asynchronously.

    This function runs in a thread pool to avoid blocking the event loop.

    Args:
        html: The HTML string to parse
        expr: XPath expression string
        **kwargs: Additional arguments (max_size_bytes, truncate_on_limit, etc.)

    Returns:
        The first Element matching the XPath expression, or None if no match
    """
    return await asyncio.to_thread(_xpath_first, html, expr, **kwargs)
