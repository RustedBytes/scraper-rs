from __future__ import annotations

from typing import TypedDict

class ElementDict(TypedDict):
    tag: str
    text: str
    html: str
    attrs: dict[str, str]

class Element:
    tag: str
    text: str
    html: str
    attrs: dict[str, str]

    def __repr__(self) -> str: ...
    def attr(self, name: str) -> str | None: ...
    def get(self, name: str, default: str | None = ...) -> str | None: ...
    def select(self, css: str) -> list[Element]: ...
    def select_first(self, css: str) -> Element | None: ...
    def find(self, css: str) -> Element | None: ...
    def css(self, css: str) -> list[Element]: ...
    def xpath(self, expr: str) -> list[Element]: ...
    def xpath_first(self, expr: str) -> Element | None: ...
    def to_dict(self) -> ElementDict: ...

class Document:
    html: str
    text: str

    def __init__(
        self,
        html: str,
        *,
        max_size_bytes: int | None = ...,
        truncate_on_limit: bool = False,
    ) -> None: ...
    @classmethod
    def from_html(
        cls,
        html: str,
        *,
        max_size_bytes: int | None = ...,
        truncate_on_limit: bool = False,
    ) -> Document: ...
    def select(self, css: str) -> list[Element]: ...
    def select_first(self, css: str) -> Element | None: ...
    def find(self, css: str) -> Element | None: ...
    def css(self, css: str) -> list[Element]: ...
    def xpath(self, expr: str) -> list[Element]: ...
    def xpath_first(self, expr: str) -> Element | None: ...
    def close(self) -> None: ...
    def __enter__(self) -> Document: ...
    def __exit__(
        self, exc_type: type[BaseException] | None, exc: BaseException | None, traceback
    ) -> None: ...
    def __repr__(self) -> str: ...

def parse(
    html: str, *, max_size_bytes: int | None = ..., truncate_on_limit: bool = False
) -> Document: ...
def select(
    html: str,
    css: str,
    *,
    max_size_bytes: int | None = ...,
    truncate_on_limit: bool = False,
) -> list[Element]: ...
def select_first(
    html: str,
    css: str,
    *,
    max_size_bytes: int | None = ...,
    truncate_on_limit: bool = False,
) -> Element | None: ...
def first(
    html: str,
    css: str,
    *,
    max_size_bytes: int | None = ...,
    truncate_on_limit: bool = False,
) -> Element | None: ...
def xpath(
    html: str,
    expr: str,
    *,
    max_size_bytes: int | None = ...,
    truncate_on_limit: bool = False,
) -> list[Element]: ...
def xpath_first(
    html: str,
    expr: str,
    *,
    max_size_bytes: int | None = ...,
    truncate_on_limit: bool = False,
) -> Element | None: ...

__version__: str
