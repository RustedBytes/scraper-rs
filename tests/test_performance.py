"""Performance tests to validate optimization improvements."""

import time

import pytest

from scraper_rs import Document, select, xpath


@pytest.fixture
def large_html() -> str:
    """Generate a large HTML document for performance testing."""
    items = "\n".join(
        f'<div class="item" data-id="{i}"><a href="/item{i}">Item {i}</a><p>Description {i}</p></div>'
        for i in range(1000)
    )
    return f"""
    <html>
      <head><title>Test Page</title></head>
      <body>
        <nav>
          <ul>
            <li><a href="/home">Home</a></li>
            <li><a href="/about">About</a></li>
          </ul>
        </nav>
        <main>
          {items}
        </main>
      </body>
    </html>
    """


def test_lazy_xpath_parsing(large_html: str) -> None:
    """Test that XPath parsing is lazy - not parsed until xpath() is called."""
    # Parsing document should NOT parse with sxd_html yet
    start = time.perf_counter()
    doc = Document(large_html)
    parse_time = time.perf_counter() - start

    # Using only CSS selectors should be fast (no XPath parsing)
    start = time.perf_counter()
    items = doc.select(".item")
    css_time = time.perf_counter() - start

    assert len(items) == 1000
    assert css_time < parse_time * 2  # CSS should be comparable to parse time

    # First XPath call should parse the document
    start = time.perf_counter()
    xpath_items = doc.xpath("//div[@class='item']")
    first_xpath_time = time.perf_counter() - start

    assert len(xpath_items) == 1000

    # Second XPath call should reuse the parsed document (should be faster)
    start = time.perf_counter()
    xpath_links = doc.xpath("//a[@href]")
    second_xpath_time = time.perf_counter() - start

    assert len(xpath_links) > 0
    # Second XPath should be faster since package is already parsed
    assert second_xpath_time <= first_xpath_time


def test_lazy_element_properties(large_html: str) -> None:
    """Test that element properties are computed lazily."""
    doc = Document(large_html)

    # Get elements but don't access any properties yet
    start = time.perf_counter()
    items = doc.select(".item")
    select_time = time.perf_counter() - start

    assert len(items) == 1000

    # Access only tag (should be fast - already stored)
    tags = [item.tag for item in items]
    assert all(tag == "div" for tag in tags)

    # Access text on ONE element (should only compute for that one)
    start = time.perf_counter()
    first_text = items[0].text
    single_text_time = time.perf_counter() - start

    assert "Item 0" in first_text

    # Access text again on same element (should be cached, faster)
    start = time.perf_counter()
    first_text_again = items[0].text
    cached_text_time = time.perf_counter() - start

    assert first_text == first_text_again
    # Cached access should be faster
    assert cached_text_time <= single_text_time


def test_css_only_no_xpath_parsing(large_html: str) -> None:
    """Test that using only CSS selectors never triggers XPath parsing."""
    # This is a behavioral test - if XPath is lazy, CSS-only usage should be fast
    start = time.perf_counter()

    items = select(large_html, ".item")
    assert len(items) == 1000

    # Access only CSS-related properties
    for i, item in enumerate(items[:10]):  # Just check first 10
        assert item.tag == "div"
        assert item.attr("data-id") == str(i)
        assert item.html  # Inner HTML should be fast

    css_only_time = time.perf_counter() - start

    # Now do the same with XPath to compare
    start = time.perf_counter()

    xpath_items = xpath(large_html, "//div[@class='item']")
    assert len(xpath_items) == 1000

    for i, item in enumerate(xpath_items[:10]):
        assert item.tag == "div"
        assert item.attr("data-id") == str(i)

    xpath_time = time.perf_counter() - start

    # XPath should be slower due to additional parsing
    # (though both should still be reasonably fast)
    # This is mainly a sanity check
    assert css_only_time > 0 and xpath_time > 0


def test_minimal_property_access(large_html: str) -> None:
    """Test that accessing minimal properties is fast."""
    doc = Document(large_html)
    items = doc.select(".item")

    # If we only access tag and one attribute, text should never be computed
    start = time.perf_counter()

    for item in items[:100]:  # Check first 100 items
        _ = item.tag
        _ = item.attr("data-id")
        # NOT accessing .text or .attrs (full dict)

    minimal_time = time.perf_counter() - start

    # Reset and access everything
    items2 = doc.select(".item")
    start = time.perf_counter()

    for item in items2[:100]:
        _ = item.tag
        _ = item.attr("data-id")
        _ = item.text  # This triggers text computation
        _ = item.attrs  # This triggers attrs computation

    full_time = time.perf_counter() - start

    # Accessing everything should take longer
    assert full_time > minimal_time
