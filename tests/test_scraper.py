import pytest

from scraper_rs import Document, first, parse, select


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


def test_find_and_first_helpers(sample_html: str) -> None:
    doc = Document(sample_html)

    first_link = doc.find("a[href]")
    assert first_link is not None
    assert first_link.tag == "a"
    assert first_link.text == "First"
    assert first_link.attr("href") == "/a"

    assert doc.find("p") is None
    assert first(sample_html, "a[href]").attr("href") == "/a"
    assert first(sample_html, "p") is None


def test_top_level_parse_and_select(sample_html: str) -> None:
    doc = parse(sample_html)
    links = select(sample_html, "a[href]")

    assert isinstance(doc, Document)
    assert len(links) == 2
    assert [link.text for link in links] == ["First", "Second"]
    assert [link.attr("href") for link in links] == ["/a", "/b"]


def test_css_alias_and_invalid_selector(sample_html: str) -> None:
    doc = Document(sample_html)

    css_links = doc.css("a[href]")
    assert [link.text for link in css_links] == ["First", "Second"]
