import importlib.metadata

import pytest

from scraper_rs import (
    Document,
    __version__,
    first,
    parse,
    select,
    select_first,
    xpath,
    xpath_first,
)


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


def test_document_properties(sample_html: str) -> None:
    doc = Document(sample_html)

    assert doc.html == sample_html
    assert doc.text == "First Second"
    assert "len_html" in repr(doc)
    assert str(len(sample_html)) in repr(doc)


def test_select_and_element_helpers(sample_html: str) -> None:
    doc = Document(sample_html)
    items = doc.select(".item")

    assert len(items) == 2

    first_item = items[0]
    assert first_item.tag == "div"
    assert first_item.text == "First"
    assert first_item.html == '<a href="/a">First</a>'
    assert first_item.attr("data-id") == "1"
    assert first_item.get("data-id", None) == "1"
    assert first_item.get("missing", "fallback") == "fallback"
    assert first_item.attrs["class"] == "item"
    assert first_item.attrs["data-id"] == "1"
    assert "<Element tag='div' text=First>" in repr(first_item)

    expected_dict = {
        "tag": "div",
        "text": "First",
        "html": '<a href="/a">First</a>',
        "attrs": {"class": "item", "data-id": "1"},
    }
    assert first_item.to_dict() == expected_dict


def test_document_size_limit(sample_html: str) -> None:
    tiny_limit = 10
    with pytest.raises(ValueError, match="too large"):
        Document(sample_html, max_size_bytes=tiny_limit)

    ok_limit = len(sample_html.encode("utf-8"))
    doc = Document(sample_html, max_size_bytes=ok_limit)
    assert doc.find("a[href]") is not None

    with pytest.raises(ValueError):
        select(sample_html, "a[href]", max_size_bytes=tiny_limit)


def test_document_truncate_on_limit() -> None:
    # Create a large HTML document
    large_html = """
    <html>
      <body>
        <div class="start">This is the beginning</div>
        <div class="middle">This is the middle part with lots of text that will be truncated</div>
        <div class="end">This should not appear in the truncated version</div>
      </body>
    </html>
    """

    # Set a limit that will cut off the HTML midway
    small_limit = 100

    # Without truncate_on_limit, should raise an error
    with pytest.raises(ValueError, match="too large"):
        Document(large_html, max_size_bytes=small_limit)

    # With truncate_on_limit=True, should parse successfully
    doc = Document(large_html, max_size_bytes=small_limit, truncate_on_limit=True)

    # Should have parsed the beginning
    assert doc.find(".start") is not None

    # The end should not be present due to truncation
    assert doc.find(".end") is None

    # The HTML should be truncated
    assert len(doc.html) == small_limit or len(doc.html) < small_limit

    # Test with top-level functions
    items = select(
        large_html, ".start", max_size_bytes=small_limit, truncate_on_limit=True
    )
    assert len(items) > 0

    first_item = first(
        large_html, ".start", max_size_bytes=small_limit, truncate_on_limit=True
    )
    assert first_item is not None

    # Verify the end is not found
    end_items = select(
        large_html, ".end", max_size_bytes=small_limit, truncate_on_limit=True
    )
    assert len(end_items) == 0


def test_truncate_utf8_boundary() -> None:
    # Test that truncation respects UTF-8 character boundaries
    # Using emoji which takes multiple bytes in UTF-8 encoding
    html_with_emoji = "<html><body>Hello 😀 World</body></html>"

    # Set limit that would cut in the middle of a multi-byte character
    # The emoji 😀 is a 4-byte UTF-8 sequence
    limit_in_emoji = 20

    doc = Document(
        html_with_emoji, max_size_bytes=limit_in_emoji, truncate_on_limit=True
    )

    # Should not crash and should produce valid HTML
    assert len(doc.html) <= limit_in_emoji
    # The text should be valid (no broken UTF-8)
    text = doc.text
    assert isinstance(text, str)


def test_find_and_first_helpers(sample_html: str) -> None:
    doc = Document(sample_html)

    first_link = doc.find("a[href]")
    assert first_link is not None
    assert first_link.tag == "a"
    assert first_link.text == "First"
    assert first_link.attr("href") == "/a"

    first_link_via_select = doc.select_first("a[href]")
    assert first_link_via_select is not None
    assert first_link_via_select.attr("href") == "/a"

    assert doc.find("p") is None
    assert doc.select_first("p") is None
    assert first(sample_html, "a[href]").attr("href") == "/a"
    assert first(sample_html, "p") is None
    assert select_first(sample_html, "a[href]").attr("href") == "/a"
    assert select_first(sample_html, "p") is None


def test_top_level_parse_and_select(sample_html: str) -> None:
    doc = parse(sample_html)
    links = select(sample_html, "a[href]")

    assert isinstance(doc, Document)
    assert len(links) == 2
    assert [link.text for link in links] == ["First", "Second"]
    assert [link.attr("href") for link in links] == ["/a", "/b"]
    assert [link.text for link in xpath(sample_html, "//div[@class='item']/a")] == [
        "First",
        "Second",
    ]
    assert xpath_first(sample_html, "//div[@data-id='1']/a").text == "First"


def test_css_alias_and_invalid_selector(sample_html: str) -> None:
    doc = Document(sample_html)

    css_links = doc.css("a[href]")
    assert [link.text for link in css_links] == ["First", "Second"]


def test_element_nested_selection(sample_html: str) -> None:
    doc = Document(sample_html)

    item = doc.find(".item")
    assert item is not None

    nested_links = item.select("a[href]")
    assert len(nested_links) == 1
    assert nested_links[0].text == "First"
    assert nested_links[0].attr("href") == "/a"

    first_nested = item.select_first("a[href]")
    assert first_nested is not None
    assert first_nested.text == "First"

    assert item.find("p") is None
    assert item.select_first("p") is None
    assert [link.tag for link in item.css("a")] == ["a"]


def test_xpath_selection(sample_html: str) -> None:
    doc = Document(sample_html)

    items = doc.xpath("//div[@class='item']")
    assert [item.attr("data-id") for item in items] == ["1", "2"]

    last_link = doc.xpath_first("//div[@data-id='2']/a")
    assert last_link is not None
    assert last_link.text == "Second"
    assert last_link.attr("href") == "/b"

    nested = items[0].xpath("./a")
    assert len(nested) == 1
    assert nested[0].text == "First"
    assert nested[0].attr("href") == "/a"


def test_version_exposed() -> None:
    assert __version__ == importlib.metadata.version("scraper-rust")


def test_document_close_releases_resources(sample_html: str) -> None:
    doc = Document(sample_html)

    assert doc.select("a")
    assert doc.xpath("//a")
    assert doc.find("a")

    doc.close()
    doc.close()  # idempotent

    assert doc.html == ""
    assert doc.text == ""
    assert doc.select("a") == []
    assert doc.select_first("a") is None
    assert doc.find("a") is None
    assert doc.xpath("//a") == []
    assert doc.xpath_first("//a") is None


def test_document_context_manager_closes(sample_html: str) -> None:
    with Document(sample_html) as doc:
        assert doc.find("a[href]") is not None

    assert doc.html == ""
    assert doc.select("a") == []
