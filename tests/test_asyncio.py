"""Tests for the scraper_rs.asyncio module."""

import asyncio

import pytest

from scraper_rs import Document, Element
from scraper_rs import asyncio as async_scraper


@pytest.fixture
def sample_html() -> str:
    return """
    <html>
      <body>
        <div class="item" data-id="1"><a href="/a">First</a></div>
        <div class="item" data-id="2"><a href="/b">Second</a></div>
      </body>
    </html>
    """


@pytest.mark.asyncio
async def test_async_parse(sample_html: str) -> None:
    """Test the async parse function."""
    doc = await async_scraper.parse(sample_html)

    assert isinstance(doc, Document)
    assert doc.text == "First Second"
    assert doc.html == sample_html


@pytest.mark.asyncio
async def test_async_select(sample_html: str) -> None:
    """Test the async select function."""
    items = await async_scraper.select(sample_html, ".item")

    assert len(items) == 2
    assert all(isinstance(item, Element) for item in items)
    assert items[0].tag == "div"
    assert items[0].text == "First"
    assert items[0].attr("data-id") == "1"
    assert items[1].text == "Second"
    assert items[1].attr("data-id") == "2"


@pytest.mark.asyncio
async def test_async_xpath(sample_html: str) -> None:
    """Test the async xpath function."""
    links = await async_scraper.xpath(sample_html, "//a[@href]")

    assert len(links) == 2
    assert all(isinstance(link, Element) for link in links)
    assert links[0].tag == "a"
    assert links[0].text == "First"
    assert links[0].attr("href") == "/a"
    assert links[1].text == "Second"
    assert links[1].attr("href") == "/b"


@pytest.mark.asyncio
async def test_async_parse_with_max_size(sample_html: str) -> None:
    """Test async parse with max_size_bytes parameter."""
    # Should work with sufficient size
    ok_limit = len(sample_html.encode("utf-8"))
    doc = await async_scraper.parse(sample_html, max_size_bytes=ok_limit)
    assert doc.text == "First Second"

    # Should fail with tiny limit
    tiny_limit = 10
    with pytest.raises(ValueError, match="too large"):
        await async_scraper.parse(sample_html, max_size_bytes=tiny_limit)


@pytest.mark.asyncio
async def test_async_select_with_max_size(sample_html: str) -> None:
    """Test async select with max_size_bytes parameter."""
    # Should work with sufficient size
    ok_limit = len(sample_html.encode("utf-8"))
    items = await async_scraper.select(sample_html, ".item", max_size_bytes=ok_limit)
    assert len(items) == 2

    # Should fail with tiny limit
    tiny_limit = 10
    with pytest.raises(ValueError, match="too large"):
        await async_scraper.select(sample_html, ".item", max_size_bytes=tiny_limit)


@pytest.mark.asyncio
async def test_async_xpath_with_max_size(sample_html: str) -> None:
    """Test async xpath with max_size_bytes parameter."""
    # Should work with sufficient size
    ok_limit = len(sample_html.encode("utf-8"))
    links = await async_scraper.xpath(
        sample_html, "//a[@href]", max_size_bytes=ok_limit
    )
    assert len(links) == 2

    # Should fail with tiny limit
    tiny_limit = 10
    with pytest.raises(ValueError, match="too large"):
        await async_scraper.xpath(sample_html, "//a[@href]", max_size_bytes=tiny_limit)


@pytest.mark.asyncio
async def test_async_with_truncate_on_limit() -> None:
    """Test async functions with truncate_on_limit parameter."""
    large_html = """
    <html>
      <body>
        <div class="start">This is the beginning</div>
        <div class="middle">This is the middle part with lots of text</div>
        <div class="end">This should not appear in truncated version</div>
      </body>
    </html>
    """

    small_limit = 100

    # Test parse with truncate_on_limit
    doc = await async_scraper.parse(
        large_html, max_size_bytes=small_limit, truncate_on_limit=True
    )
    assert doc.find(".start") is not None
    assert doc.find(".end") is None

    # Test select with truncate_on_limit
    items = await async_scraper.select(
        large_html, ".start", max_size_bytes=small_limit, truncate_on_limit=True
    )
    assert len(items) > 0

    # Test xpath with truncate_on_limit
    start_elements = await async_scraper.xpath(
        large_html,
        "//div[@class='start']",
        max_size_bytes=small_limit,
        truncate_on_limit=True,
    )
    assert len(start_elements) > 0


@pytest.mark.asyncio
async def test_async_xpath_first(sample_html: str) -> None:
    """Test the async xpath_first function."""
    # Find the first link by XPath
    first_link = await async_scraper.xpath_first(sample_html, "//a[@href]")

    assert first_link is not None
    assert isinstance(first_link, Element)
    assert first_link.tag == "a"
    assert first_link.text == "First"
    assert first_link.attr("href") == "/a"

    # Find a specific link
    second_link = await async_scraper.xpath_first(sample_html, "//div[@data-id='2']/a")
    assert second_link is not None
    assert second_link.text == "Second"
    assert second_link.attr("href") == "/b"

    # Test with non-matching XPath (should return None)
    no_match = await async_scraper.xpath_first(sample_html, "//p[@class='missing']")
    assert no_match is None


@pytest.mark.asyncio
async def test_async_xpath_first_with_max_size(sample_html: str) -> None:
    """Test async xpath_first with max_size_bytes parameter."""
    # Should work with sufficient size
    ok_limit = len(sample_html.encode("utf-8"))
    first_link = await async_scraper.xpath_first(
        sample_html, "//a[@href]", max_size_bytes=ok_limit
    )
    assert first_link is not None
    assert first_link.text == "First"

    # Should fail with tiny limit
    tiny_limit = 10
    with pytest.raises(ValueError, match="too large"):
        await async_scraper.xpath_first(
            sample_html, "//a[@href]", max_size_bytes=tiny_limit
        )


@pytest.mark.asyncio
async def test_async_select_first(sample_html: str) -> None:
    """Test the async select_first function."""
    # Find the first item by CSS selector
    first_item = await async_scraper.select_first(sample_html, ".item")

    assert first_item is not None
    assert isinstance(first_item, Element)
    assert first_item.tag == "div"
    assert first_item.text == "First"
    assert first_item.attr("data-id") == "1"

    # Find a specific element
    second_link = await async_scraper.select_first(sample_html, "div[data-id='2'] a")
    assert second_link is not None
    assert second_link.text == "Second"
    assert second_link.attr("href") == "/b"

    # Test with non-matching selector (should return None)
    no_match = await async_scraper.select_first(sample_html, "p.missing")
    assert no_match is None


@pytest.mark.asyncio
async def test_async_select_first_with_max_size(sample_html: str) -> None:
    """Test async select_first with max_size_bytes parameter."""
    # Should work with sufficient size
    ok_limit = len(sample_html.encode("utf-8"))
    first_item = await async_scraper.select_first(
        sample_html, ".item", max_size_bytes=ok_limit
    )
    assert first_item is not None
    assert first_item.text == "First"

    # Should fail with tiny limit
    tiny_limit = 10
    with pytest.raises(ValueError, match="too large"):
        await async_scraper.select_first(
            sample_html, ".item", max_size_bytes=tiny_limit
        )


@pytest.mark.asyncio
async def test_multiple_async_calls_concurrently(sample_html: str) -> None:
    """Test that multiple async calls can be made concurrently."""
    # Run multiple async operations concurrently
    results = await asyncio.gather(
        async_scraper.select(sample_html, ".item"),
        async_scraper.xpath(sample_html, "//a[@href]"),
        async_scraper.parse(sample_html),
    )

    items, links, doc = results

    assert len(items) == 2
    assert len(links) == 2
    assert isinstance(doc, Document)
    assert doc.text == "First Second"
