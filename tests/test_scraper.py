import importlib.metadata

import pytest

from scraper_rs import (
    Document,
    __version__,
    first,
    parse,
    parse_document,
    parse_fragment,
    prettify,
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


def test_prettify_document_and_top_level(sample_html: str) -> None:
    doc = Document(sample_html)

    pretty_via_doc = doc.prettify()
    pretty_via_function = prettify(sample_html)

    assert pretty_via_doc == pretty_via_function
    assert pretty_via_doc.startswith("<html>")
    assert "  <body>" in pretty_via_doc
    assert '    <div class="item" data-id="1">' in pretty_via_doc
    assert '      <a href="/a">First</a>' in pretty_via_doc


def test_prettify_element(sample_html: str) -> None:
    doc = Document(sample_html)
    first_item = doc.select_first(".item")

    assert first_item is not None
    pretty_item = first_item.prettify()
    assert pretty_item.startswith('<div class="item" data-id="1">')
    assert pretty_item.endswith("</div>")
    assert '  <a href="/a">First</a>' in pretty_item


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


def test_parse_document_to_dict(sample_html: str) -> None:
    parsed = parse_document(sample_html)

    assert parsed["node_type"] == "document"
    assert parsed["quirks_mode"] == "no-quirks"
    assert parsed["errors"] == []

    html_node = next(
        child
        for child in parsed["children"]
        if child["node_type"] == "element" and child["tag"] == "html"
    )
    body_node = next(
        child
        for child in html_node["children"]
        if child["node_type"] == "element" and child["tag"] == "body"
    )
    first_div = next(
        child
        for child in body_node["children"]
        if child["node_type"] == "element" and child["tag"] == "div"
    )

    assert first_div["attrs"]["class"] == "item"
    assert first_div["attrs"]["data-id"] == "1"


def test_parse_fragment_to_dict() -> None:
    parsed = parse_fragment(
        '<div class="item">Hello <span>world</span><!-- note --></div>'
    )

    assert parsed["node_type"] == "document_fragment"
    assert parsed["errors"] == []

    div_node = next(
        child
        for child in parsed["children"]
        if child["node_type"] == "element" and child["tag"] == "div"
    )

    assert div_node["attrs"]["class"] == "item"
    assert div_node["children"][0]["node_type"] == "text"
    assert div_node["children"][0]["text"] == "Hello "
    assert div_node["children"][1]["tag"] == "span"
    assert div_node["children"][1]["children"][0]["text"] == "world"
    assert div_node["children"][2]["node_type"] == "comment"
    assert div_node["children"][2]["text"] == " note "


def test_parse_tree_attrs_and_sibling_text_order() -> None:
    parsed = parse_fragment(
        '<input disabled id="enabled"><p>first <b>bold</b> second</p>'
    )

    input_node = next(
        child
        for child in parsed["children"]
        if child["node_type"] == "element" and child["tag"] == "input"
    )
    assert input_node["attrs"]["disabled"] == ""
    assert input_node["attrs"]["id"] == "enabled"

    paragraph = next(
        child
        for child in parsed["children"]
        if child["node_type"] == "element" and child["tag"] == "p"
    )
    assert [child["node_type"] for child in paragraph["children"]] == [
        "text",
        "element",
        "text",
    ]
    assert paragraph["children"][0]["text"] == "first "
    assert paragraph["children"][1]["tag"] == "b"
    assert paragraph["children"][1]["children"][0]["text"] == "bold"
    assert paragraph["children"][2]["text"] == " second"


def test_parse_tree_helpers_respect_size_limits() -> None:
    html = "<div>ééééé</div>"

    with pytest.raises(ValueError, match="too large"):
        parse_document(html, max_size_bytes=5)

    parsed = parse_fragment(html, max_size_bytes=8, truncate_on_limit=True)

    assert parsed["node_type"] == "document_fragment"
    assert parsed["errors"] == []


def test_css_alias_and_invalid_selector(sample_html: str) -> None:
    doc = Document(sample_html)

    css_links = doc.css("a[href]")
    assert [link.text for link in css_links] == ["First", "Second"]

    descendant = doc.select("div[data-id='2'] a")
    assert len(descendant) == 1
    assert descendant[0].text == "Second"

    direct_children = doc.select("div > a")
    assert [link.attr("href") for link in direct_children] == ["/a", "/b"]

    comma_matches = doc.select("a, div")
    assert [element.tag for element in comma_matches] == ["div", "a", "div", "a"]

    with pytest.raises(ValueError, match="Invalid CSS selector"):
        doc.select("div[")


def test_generate_css_and_xpath_selectors(sample_html: str) -> None:
    doc = Document(sample_html)
    target = doc.select("div[data-id='2'] a")[0]

    css = doc.generate_css_selector(target)
    xpath_selector = doc.generate_xpath_selector(target)

    assert css is not None
    css_matches = doc.select(css)
    assert len(css_matches) == 1
    assert css_matches[0].attr("href") == "/b"

    assert xpath_selector is not None
    assert xpath_selector.startswith("//")
    xpath_match = doc.xpath_first(xpath_selector)
    assert xpath_match is not None
    assert xpath_match.attr("href") == "/b"


def test_generate_xpath_prefers_robust_semantic_locator() -> None:
    doc = Document(
        '<section data-testid="product-card"><span data-testid="price">$19</span></section>'
    )
    target = doc.select_first('[data-testid="price"]')

    assert target is not None
    xpath_selector = doc.generate_xpath_selector(target)

    assert xpath_selector is not None
    assert "data-testid" in xpath_selector
    match = doc.xpath_first(xpath_selector)
    assert match is not None
    assert match.text == "$19"


def test_generate_xpath_prefers_href_for_links_and_survives_dom_change() -> None:
    html_a = """
    <html><body><main>
      <aside id="most-read">
        <h2>Most read</h2>
        <ol>
          <li><a href="/news/popular/a">First</a></li>
          <li><a href="/news/popular/b">Second</a></li>
        </ol>
      </aside>
    </main></body></html>
    """

    html_b = """
    <html><body>
      <div class="page-wrapper">
        <main>
          <section class="breaking">Breaking news</section>
          <aside id="most-read">
            <div class="widget-header"><h2>Most read</h2></div>
            <ol>
              <li><a href="/news/popular/new">New</a></li>
              <li><a href="/news/popular/a">First</a></li>
              <li><a href="/news/popular/b">Second</a></li>
            </ol>
          </aside>
        </main>
      </div>
    </body></html>
    """

    doc_a = Document(html_a)
    target = doc_a.select_first('a[href="/news/popular/b"]')

    assert target is not None

    xpath_selector = doc_a.generate_xpath_selector(target)
    assert xpath_selector is not None
    assert "/news/popular/b" in xpath_selector

    doc_b = Document(html_b)
    matches = doc_b.xpath(xpath_selector)

    assert len(matches) == 1
    assert matches[0].attr("href") == "/news/popular/b"
    assert matches[0].text == "Second"


def test_generate_xpath_does_not_use_href_on_non_link_elements() -> None:
    doc = Document('<div href="/not-a-link">Target</div>')
    target = doc.select_first("div")

    assert target is not None
    xpath_selector = doc.generate_xpath_selector(target)

    assert xpath_selector is not None
    assert "@href=" not in xpath_selector


def test_generate_xpath_when_css_subset_cannot_distinguish_siblings() -> None:
    doc = Document("<ul><li>First</li><li>Second</li></ul>")
    target = doc.select("li")[1]

    assert doc.generate_css_selector(target) is None

    xpath_selector = doc.generate_xpath_selector(target)
    assert xpath_selector is not None
    match = doc.xpath_first(xpath_selector)
    assert match is not None
    assert match.text == "Second"


def test_selector_generation_rejects_detached_or_foreign_elements() -> None:
    first_doc = Document("<div><span id='a'>A</span></div>")
    second_doc = Document("<div><span id='b'>B</span></div>")

    first = first_doc.find("span")
    foreign = second_doc.find("span")
    container = first_doc.find("div")
    assert container is not None
    detached = container.select_first("span")

    assert first is not None
    assert foreign is not None

    with pytest.raises(ValueError, match="different Document"):
        first_doc.generate_css_selector(foreign)

    # Nested Element selection parses an independent fragment and therefore has
    # no stable handle back into the original document.
    if detached is not None:
        with pytest.raises(ValueError, match="requires an Element returned"):
            first_doc.generate_xpath_selector(detached)


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


def test_xpath_accepts_normal_web_page_html() -> None:
    html = """<!DOCTYPE html>
    <html lang="en">
      <head>
        <meta charset="UTF-8">
        <link rel="stylesheet" href="/main.css">
        <script>if (1 < 2 && 3 > 2) { window.ok = true; }</script>
      </head>
      <body>
        <div class="quote" itemscope>
          <span class="text">A &amp; B</span>
          <meta class="keywords" content="example">
        </div>
      </body>
    </html>"""

    doc = Document(html)
    quotes = doc.xpath("//div[@class='quote']")
    assert len(quotes) == 1
    assert quotes[0].xpath_first(".//span[@class='text']").text == "A & B"

    assert len(xpath(html, "//div[@class='quote']")) == 1
    assert xpath_first(html, "//meta[@class='keywords']") is not None


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
    assert doc.prettify() == ""


def test_document_context_manager_closes(sample_html: str) -> None:
    with Document(sample_html) as doc:
        assert doc.find("a[href]") is not None

    assert doc.html == ""
    assert doc.select("a") == []


def test_text_and_attributes_decode_character_references() -> None:
    html = (
        '<div><a href="/p?a=1&amp;b=2&not=3" title="&quot;Q&quot; &#x26; A">'
        "Fish &amp; Chips &mdash; &#169;</a>"
        "<script>if (a &amp;&amp; b) {}</script></div>"
    )
    doc = Document(html)
    link = doc.select_first("a")

    assert link.attr("href") == "/p?a=1&b=2&not=3"
    assert link.get("title", None) == '"Q" & A'
    assert link.attrs["href"] == "/p?a=1&b=2&not=3"
    assert link.text == "Fish & Chips \u2014 \u00a9"
    assert doc.select_first("script").text == "if (a &amp;&amp; b) {}"
    assert doc.xpath_first("//a").attr("href") == "/p?a=1&b=2&not=3"


def test_dict_tree_decodes_text_and_attributes_but_not_raw_text() -> None:
    parsed = parse_fragment(
        '<p title="a &amp; b">x &lt; y<script>1 &amp;&amp; 2</script></p>'
    )
    paragraph = parsed["children"][0]
    text_node, script = paragraph["children"]

    assert paragraph["attrs"]["title"] == "a & b"
    assert text_node["text"] == "x < y"
    assert script["children"][0]["text"] == "1 &amp;&amp; 2"
