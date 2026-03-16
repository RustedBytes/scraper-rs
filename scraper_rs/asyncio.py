"""Asyncio wrappers for scraper_rs functions."""

from __future__ import annotations

import asyncio

from .scraper_rs import (
    Document as _Document,
    Element as _Element,
    first as _first_sync,
    prettify as _prettify_sync,
    select as _select_sync,
    select_first as _select_first_sync,
    xpath as _xpath_sync,
    xpath_first as _xpath_first_sync,
)


def _wrap_element(element: _Element | None) -> "AsyncElement | None":
    if element is None:
        return None
    return AsyncElement(element)


def _wrap_elements(elements: list[_Element]) -> list["AsyncElement"]:
    return [AsyncElement(element) for element in elements]


def _parse_state(html: str, kwargs: dict[str, int | bool | None]) -> tuple[str, str]:
    document = _Document(html, **kwargs)
    try:
        return document.html, document.text
    finally:
        document.close()


class AsyncElement:
    """Async wrapper for immutable element snapshots."""

    __slots__ = ("_element",)

    def __init__(self, element: _Element) -> None:
        self._element = element

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
        await asyncio.sleep(0)
        return _wrap_elements(self._element.select(css))

    async def select_first(self, css: str) -> "AsyncElement | None":
        await asyncio.sleep(0)
        return _wrap_element(self._element.select_first(css))

    async def find(self, css: str) -> "AsyncElement | None":
        await asyncio.sleep(0)
        return _wrap_element(self._element.find(css))

    async def css(self, css: str) -> list["AsyncElement"]:
        await asyncio.sleep(0)
        return _wrap_elements(self._element.css(css))

    async def xpath(self, expr: str) -> list["AsyncElement"]:
        await asyncio.sleep(0)
        return _wrap_elements(self._element.xpath(expr))

    async def xpath_first(self, expr: str) -> "AsyncElement | None":
        await asyncio.sleep(0)
        return _wrap_element(self._element.xpath_first(expr))

    async def prettify(self) -> str:
        await asyncio.sleep(0)
        return self._element.prettify()

    def to_dict(self) -> dict[str, str | dict[str, str]]:
        return self._element.to_dict()

    def __repr__(self) -> str:
        return repr(self._element)


class AsyncDocument:
    """Async wrapper that stores only shareable document state."""

    __slots__ = ("_html", "_text", "_closed")

    def __init__(self, html: str, text: str) -> None:
        self._html = html
        self._text = text
        self._closed = False

    @property
    def html(self) -> str:
        return "" if self._closed else self._html

    @property
    def text(self) -> str:
        return "" if self._closed else self._text

    async def select(self, css: str) -> list[AsyncElement]:
        if self._closed:
            return []
        return await select(self._html, css)

    async def select_first(self, css: str) -> AsyncElement | None:
        if self._closed:
            return None
        return await select_first(self._html, css)

    async def find(self, css: str) -> AsyncElement | None:
        return await self.select_first(css)

    async def css(self, css: str) -> list[AsyncElement]:
        return await self.select(css)

    async def xpath(self, expr: str) -> list[AsyncElement]:
        if self._closed:
            return []
        return await xpath(self._html, expr)

    async def xpath_first(self, expr: str) -> AsyncElement | None:
        if self._closed:
            return None
        return await xpath_first(self._html, expr)

    async def prettify(self) -> str:
        if self._closed:
            return ""
        await asyncio.sleep(0)
        return _prettify_sync(self._html)

    def close(self) -> None:
        self._closed = True
        self._html = ""
        self._text = ""

    def __enter__(self) -> "AsyncDocument":
        return self

    def __exit__(self, exc_type, exc_value, traceback) -> None:
        self.close()

    async def __aenter__(self) -> "AsyncDocument":
        return self

    async def __aexit__(self, exc_type, exc_value, traceback) -> None:
        self.close()

    def __repr__(self) -> str:
        return f"<AsyncDocument len_html={len(self.html)}>"


async def parse(html: str, **kwargs) -> "AsyncDocument":
    await asyncio.sleep(0)
    parsed_html, text = _parse_state(html, kwargs)
    return AsyncDocument(parsed_html, text)


async def select(html: str, css: str, **kwargs) -> list["AsyncElement"]:
    await asyncio.sleep(0)
    return _wrap_elements(_select_sync(html, css, **kwargs))


async def select_first(html: str, css: str, **kwargs) -> "AsyncElement | None":
    await asyncio.sleep(0)
    return _wrap_element(_select_first_sync(html, css, **kwargs))


async def first(html: str, css: str, **kwargs) -> "AsyncElement | None":
    await asyncio.sleep(0)
    return _wrap_element(_first_sync(html, css, **kwargs))


async def xpath(html: str, expr: str, **kwargs) -> list["AsyncElement"]:
    await asyncio.sleep(0)
    return _wrap_elements(_xpath_sync(html, expr, **kwargs))


async def xpath_first(html: str, expr: str, **kwargs) -> "AsyncElement | None":
    await asyncio.sleep(0)
    return _wrap_element(_xpath_first_sync(html, expr, **kwargs))


async def prettify(html: str, **kwargs) -> str:
    await asyncio.sleep(0)
    return _prettify_sync(html, **kwargs)


__all__ = [
    "AsyncDocument",
    "AsyncElement",
    "first",
    "parse",
    "prettify",
    "select",
    "select_first",
    "xpath",
    "xpath_first",
]
