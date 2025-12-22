"""Asyncio wrappers for scraper_rs functions.

This module provides async versions of the main scraper_rs functions,
including awaitable selectors on nested elements.
"""

import asyncio

from .scraper_rs import (
    Document as _Document,
    Element as _Element,
    _select_first_fragment_async,
    _select_fragment_async,
    _xpath_first_fragment_async,
    _xpath_fragment_async,
    first_async as _first_async,
    select_async as _select_async,
    select_first_async as _select_first_async,
    xpath_async as _xpath_async,
    xpath_first_async as _xpath_first_async,
)


def _wrap_element(element: _Element | None) -> "AsyncElement | None":
    if element is None:
        return None
    return AsyncElement(element)


def _wrap_elements(elements: list[_Element]) -> list["AsyncElement"]:
    return [AsyncElement(element) for element in elements]


class AsyncElement:
    """Async wrapper for Element objects with awaitable selectors."""

    __slots__ = ("_element",)

    def __init__(self, element: _Element) -> None:
        self._element = element

    @property
    def element(self) -> _Element:
        return self._element

    @property
    def tag(self) -> str:
        return self._element.tag

    @property
    def text(self) -> str:
        return self._element.text

    @property
    def html(self) -> str:
        return self._element.html

    @property
    def attrs(self) -> dict[str, str]:
        return self._element.attrs

    def attr(self, name: str) -> str | None:
        return self._element.attr(name)

    def get(self, name: str, default: str | None = None) -> str | None:
        return self._element.get(name, default)

    async def select(self, css: str) -> list["AsyncElement"]:
        return _wrap_elements(await _select_fragment_async(self._element.html, css))

    async def select_first(self, css: str) -> "AsyncElement | None":
        return _wrap_element(
            await _select_first_fragment_async(self._element.html, css)
        )

    async def find(self, css: str) -> "AsyncElement | None":
        return await self.select_first(css)

    async def css(self, css: str) -> list["AsyncElement"]:
        return await self.select(css)

    async def xpath(self, expr: str) -> list["AsyncElement"]:
        return _wrap_elements(await _xpath_fragment_async(self._element.html, expr))

    async def xpath_first(self, expr: str) -> "AsyncElement | None":
        return _wrap_element(
            await _xpath_first_fragment_async(self._element.html, expr)
        )

    def to_dict(self) -> dict[str, str | dict[str, str]]:
        return self._element.to_dict()

    def __repr__(self) -> str:
        return repr(self._element)


class AsyncDocument:
    """Async wrapper for Document objects with awaitable selectors."""

    __slots__ = ("_document",)

    def __init__(self, document: _Document) -> None:
        self._document = document

    @property
    def document(self) -> _Document:
        return self._document

    @property
    def html(self) -> str:
        return self._document.html

    @property
    def text(self) -> str:
        return self._document.text

    async def select(self, css: str) -> list[AsyncElement]:
        return _wrap_elements(await _select_async(self._document.html, css))

    async def select_first(self, css: str) -> AsyncElement | None:
        return _wrap_element(await _select_first_async(self._document.html, css))

    async def find(self, css: str) -> AsyncElement | None:
        return await self.select_first(css)

    async def css(self, css: str) -> list[AsyncElement]:
        return await self.select(css)

    async def xpath(self, expr: str) -> list[AsyncElement]:
        return _wrap_elements(await _xpath_async(self._document.html, expr))

    async def xpath_first(self, expr: str) -> AsyncElement | None:
        return _wrap_element(await _xpath_first_async(self._document.html, expr))

    def close(self) -> None:
        self._document.close()

    def __enter__(self) -> "AsyncDocument":
        self._document.__enter__()
        return self

    def __exit__(self, exc_type, exc_value, traceback) -> None:
        self._document.__exit__(exc_type, exc_value, traceback)

    def __repr__(self) -> str:
        return f"<AsyncDocument {self._document!r}>"


async def parse(html: str, **kwargs) -> "AsyncDocument":
    """Parse HTML asynchronously.

    Note: Due to PyO3 limitations, the Document is created in the current thread
    but yields control to the event loop to avoid blocking.

    Args:
        html: The HTML string to parse
        **kwargs: Additional arguments (max_size_bytes, truncate_on_limit, etc.)

    Returns:
        An AsyncDocument wrapper
    """
    await asyncio.sleep(0)
    return AsyncDocument(_Document(html, **kwargs))


async def select(html: str, css: str, **kwargs) -> list["AsyncElement"]:
    """Select elements by CSS selector asynchronously.

    This function uses pyo3-async-runtimes to run in a thread pool while
    properly maintaining the Python asyncio context.

    Args:
        html: The HTML string to parse
        css: CSS selector string
        **kwargs: Additional arguments (max_size_bytes, truncate_on_limit, etc.)

    Returns:
        A list of AsyncElement wrappers matching the CSS selector
    """
    return _wrap_elements(await _select_async(html, css, **kwargs))


async def select_first(html: str, css: str, **kwargs) -> "AsyncElement | None":
    """Select the first element by CSS selector asynchronously.

    This function uses pyo3-async-runtimes to run in a thread pool while
    properly maintaining the Python asyncio context.

    Args:
        html: The HTML string to parse
        css: CSS selector string
        **kwargs: Additional arguments (max_size_bytes, truncate_on_limit, etc.)

    Returns:
        The first AsyncElement matching the CSS selector, or None if no match
    """
    return _wrap_element(await _select_first_async(html, css, **kwargs))


async def first(html: str, css: str, **kwargs) -> "AsyncElement | None":
    """Alias for select_first - select the first element by CSS selector asynchronously.

    This function uses pyo3-async-runtimes to run in a thread pool while
    properly maintaining the Python asyncio context.

    Args:
        html: The HTML string to parse
        css: CSS selector string
        **kwargs: Additional arguments (max_size_bytes, truncate_on_limit, etc.)

    Returns:
        The first AsyncElement matching the CSS selector, or None if no match
    """
    return _wrap_element(await _first_async(html, css, **kwargs))


async def xpath(html: str, expr: str, **kwargs) -> list["AsyncElement"]:
    """Select elements by XPath expression asynchronously.

    This function uses pyo3-async-runtimes to run in a thread pool while
    properly maintaining the Python asyncio context.

    Args:
        html: The HTML string to parse
        expr: XPath expression string
        **kwargs: Additional arguments (max_size_bytes, truncate_on_limit, etc.)

    Returns:
        A list of AsyncElement wrappers matching the XPath expression
    """
    return _wrap_elements(await _xpath_async(html, expr, **kwargs))


async def xpath_first(html: str, expr: str, **kwargs) -> "AsyncElement | None":
    """Select the first element by XPath expression asynchronously.

    This function uses pyo3-async-runtimes to run in a thread pool while
    properly maintaining the Python asyncio context.

    Args:
        html: The HTML string to parse
        expr: XPath expression string
        **kwargs: Additional arguments (max_size_bytes, truncate_on_limit, etc.)

    Returns:
        The first AsyncElement matching the XPath expression, or None if no match
    """
    return _wrap_element(await _xpath_first_async(html, expr, **kwargs))


__all__ = [
    "AsyncDocument",
    "AsyncElement",
    "first",
    "parse",
    "select",
    "select_first",
    "xpath",
    "xpath_first",
]
