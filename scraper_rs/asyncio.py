"""Asyncio wrappers for scraper_rs functions.

This module provides async versions of the main scraper_rs functions,
allowing them to be used in asyncio applications without blocking the event loop.

The async functions use pyo3-async-runtimes to properly capture the Python event loop
and execute work in a thread pool while maintaining proper async context.
"""

import asyncio
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from . import Document, Element

# Import the synchronous functions from the main module
from . import Document as _Document
# Import the native async functions from the Rust module
from . import (
    select_async as _select_async,
    select_first_async as _select_first_async,
    xpath_async as _xpath_async,
    xpath_first_async as _xpath_first_async,
)


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

    This function uses pyo3-async-runtimes to run in a thread pool while
    properly maintaining the Python asyncio context.

    Args:
        html: The HTML string to parse
        css: CSS selector string
        **kwargs: Additional arguments (max_size_bytes, truncate_on_limit, etc.)

    Returns:
        A list of Element objects matching the CSS selector
    """
    return await _select_async(html, css, **kwargs)


async def select_first(html: str, css: str, **kwargs) -> "Element | None":
    """Select the first element by CSS selector asynchronously.

    This function uses pyo3-async-runtimes to run in a thread pool while
    properly maintaining the Python asyncio context.

    Args:
        html: The HTML string to parse
        css: CSS selector string
        **kwargs: Additional arguments (max_size_bytes, truncate_on_limit, etc.)

    Returns:
        The first Element matching the CSS selector, or None if no match
    """
    return await _select_first_async(html, css, **kwargs)


async def xpath(html: str, expr: str, **kwargs) -> list["Element"]:
    """Select elements by XPath expression asynchronously.

    This function uses pyo3-async-runtimes to run in a thread pool while
    properly maintaining the Python asyncio context.

    Args:
        html: The HTML string to parse
        expr: XPath expression string
        **kwargs: Additional arguments (max_size_bytes, truncate_on_limit, etc.)

    Returns:
        A list of Element objects matching the XPath expression
    """
    return await _xpath_async(html, expr, **kwargs)


async def xpath_first(html: str, expr: str, **kwargs) -> "Element | None":
    """Select the first element by XPath expression asynchronously.

    This function uses pyo3-async-runtimes to run in a thread pool while
    properly maintaining the Python asyncio context.

    Args:
        html: The HTML string to parse
        expr: XPath expression string
        **kwargs: Additional arguments (max_size_bytes, truncate_on_limit, etc.)

    Returns:
        The first Element matching the XPath expression, or None if no match
    """
    return await _xpath_first_async(html, expr, **kwargs)
