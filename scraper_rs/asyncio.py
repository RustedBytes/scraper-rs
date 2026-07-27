"""Asyncio wrappers for scraper_rs functions."""

from __future__ import annotations

from . import scraper_rs as _core

_AsyncDocumentCore = _core._AsyncDocumentCore
_AsyncElementCore = _core._AsyncElementCore
_first_async = _core.first_async
_parse_async = _core.parse_async
_prettify_async = _core.prettify_async
_select_async = _core.select_async
_select_first_async = _core.select_first_async
_xpath_async = _core.xpath_async
_xpath_first_async = _core.xpath_first_async


def _wrap_element(element: _AsyncElementCore | None) -> "AsyncElement | None":
    if element is None:
        return None
    return AsyncElement(element)


def _wrap_elements(elements: list[_AsyncElementCore]) -> list["AsyncElement"]:
    return [AsyncElement(element) for element in elements]


class AsyncElement:
    """Async wrapper for immutable element snapshots."""

    __slots__ = ("_element",)

    def __init__(self, element: _AsyncElementCore) -> None:
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
        return _wrap_elements(await self._element.select(css))

    async def select_first(self, css: str) -> "AsyncElement | None":
        return _wrap_element(await self._element.select_first(css))

    async def find(self, css: str) -> "AsyncElement | None":
        return _wrap_element(await self._element.find(css))

    async def css(self, css: str) -> list["AsyncElement"]:
        return _wrap_elements(await self._element.css(css))

    async def xpath(self, expr: str) -> list["AsyncElement"]:
        return _wrap_elements(await self._element.xpath(expr))

    async def xpath_first(self, expr: str) -> "AsyncElement | None":
        return _wrap_element(await self._element.xpath_first(expr))

    async def prettify(self) -> str:
        return await self._element.prettify()

    def to_dict(self) -> dict[str, str | dict[str, str]]:
        return self._element.to_dict()

    def __repr__(self) -> str:
        return repr(self._element)


class AsyncDocument:
    """Async wrapper around Rust-owned document state."""

    __slots__ = ("_document",)

    def __init__(self, document: _AsyncDocumentCore) -> None:
        self._document = document

    @property
    def html(self) -> str:
        return self._document.html

    @property
    def text(self) -> str:
        return self._document.text

    async def select(self, css: str) -> list[AsyncElement]:
        return _wrap_elements(await self._document.select(css))

    async def select_first(self, css: str) -> AsyncElement | None:
        return _wrap_element(await self._document.select_first(css))

    async def find(self, css: str) -> AsyncElement | None:
        return _wrap_element(await self._document.find(css))

    async def css(self, css: str) -> list[AsyncElement]:
        return _wrap_elements(await self._document.css(css))

    async def xpath(self, expr: str) -> list[AsyncElement]:
        return _wrap_elements(await self._document.xpath(expr))

    async def xpath_first(self, expr: str) -> AsyncElement | None:
        return _wrap_element(await self._document.xpath_first(expr))

    async def prettify(self) -> str:
        return await self._document.prettify()

    def close(self) -> None:
        self._document.close()

    def __enter__(self) -> "AsyncDocument":
        return self

    def __exit__(self, exc_type, exc_value, traceback) -> None:
        self.close()

    async def __aenter__(self) -> "AsyncDocument":
        return self

    async def __aexit__(self, exc_type, exc_value, traceback) -> None:
        self.close()

    def __repr__(self) -> str:
        return repr(self._document)


async def parse(html: str, **kwargs) -> "AsyncDocument":
    return AsyncDocument(await _parse_async(html, **kwargs))


async def select(html: str, css: str, **kwargs) -> list["AsyncElement"]:
    return _wrap_elements(await _select_async(html, css, **kwargs))


async def select_first(html: str, css: str, **kwargs) -> "AsyncElement | None":
    return _wrap_element(await _select_first_async(html, css, **kwargs))


async def first(html: str, css: str, **kwargs) -> "AsyncElement | None":
    return _wrap_element(await _first_async(html, css, **kwargs))


async def xpath(html: str, expr: str, **kwargs) -> list["AsyncElement"]:
    return _wrap_elements(await _xpath_async(html, expr, **kwargs))


async def xpath_first(html: str, expr: str, **kwargs) -> "AsyncElement | None":
    return _wrap_element(await _xpath_first_async(html, expr, **kwargs))


async def prettify(html: str, **kwargs) -> str:
    return await _prettify_async(html, **kwargs)


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
