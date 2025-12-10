"""Type stubs for scraper_rs.asyncio module."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from . import Document, Element

async def parse(
    html: str,
    *,
    max_size_bytes: int | None = ...,
    truncate_on_limit: bool = False,
) -> Document: ...
async def select(
    html: str,
    css: str,
    *,
    max_size_bytes: int | None = ...,
    truncate_on_limit: bool = False,
) -> list[Element]: ...
async def xpath(
    html: str,
    expr: str,
    *,
    max_size_bytes: int | None = ...,
    truncate_on_limit: bool = False,
) -> list[Element]: ...
