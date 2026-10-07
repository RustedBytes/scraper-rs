"""Regression coverage for escaping, target identity and fallback normalization."""

import pytest

from scraper_rs import Document


def assert_target(doc, target):
    selector = doc.generate_xpath_selector(target)
    assert selector is not None
    matches = doc.xpath(selector)
    assert len(matches) == 1
    assert matches[0].text == "target", selector
    return selector


@pytest.mark.parametrize(
    "value", ["a&quot;b&#39;c", "a\\b", "a&#10;b", "українська", "a&amp;b"]
)
def test_xpath_attribute_escaping(value):
    doc = Document(
        f'<main><span title="{value}">target</span><span>decoy</span></main>'
    )
    assert_target(doc, doc.select("span")[0])


def test_xpath_invalid_attribute_name_falls_back():
    doc = Document('<main><span 1bad="x">target</span><span>decoy</span></main>')
    assert_target(doc, doc.select("span")[0])


def test_xpath_normalized_tag_collision():
    doc = Document("<main><a:b>decoy</a:b><a_b>target</a_b></main>")
    assert_target(doc, doc.select_first("a_b"))


def test_xpath_synthetic_wrapper_is_not_target():
    doc = Document("<xpath-document>target</xpath-document><span>decoy</span>")
    assert_target(doc, doc.select_first("xpath-document"))


def test_absolute_fallback_multiple_roots():
    doc = Document("<ul><li>decoy</li></ul><ul><li>target</li></ul>")
    assert assert_target(doc, doc.select("li")[1]).startswith("/")


@pytest.mark.parametrize(
    "value", ["a&quot;b", "a\\b", "a&#10;b", "a&amp;b", "українська"]
)
def test_css_escaping_never_returns_wrong_target(value):
    doc = Document(
        f'<main><span title="{value}">target</span><span>decoy</span></main>'
    )
    target = doc.select("span")[0]
    selector = doc.generate_css_selector(target)
    # None is valid when the native CSS subset cannot express an escaped value.
    if selector is not None:
        matches = doc.select(selector)
        assert len(matches) == 1 and matches[0].text == "target"


def test_closed_document_generation():
    doc = Document("<span>target</span>")
    target = doc.select_first("span")
    doc.close()
    assert doc.generate_css_selector(target) is None
    assert doc.generate_xpath_selector(target) is None


@pytest.mark.parametrize("depth,attributes", [(4, 128), (64, 16)])
def test_xpath_attribute_heavy_absolute_fallback(depth, attributes):
    from benchmarks.bench_selectors import fixture

    doc = Document(fixture(depth, attributes))
    target = doc.select("span")[1]
    selector = assert_target(doc, target)
    assert selector.startswith("/") and not selector.startswith("//")
    # Repeated generation must keep selecting the same target after exhaustion.
    assert doc.generate_xpath_selector(target) == selector


@pytest.mark.parametrize("attributes", [0, 128])
def test_xpath_ancestor_anchor_survives_wrappers_and_sibling_insertion(attributes):
    padding = " ".join(f'data-a{a}="same-{a}"' for a in range(attributes))
    doc = Document(
        f'<main><section data-testid="chosen"><div><span {padding} title="same">target</span>'
        f'</div></section><section><div><span {padding} title="same">decoy</span>'
        "</div></section></main>"
    )
    selector = assert_target(doc, doc.select("span")[0])
    assert "data-testid" in selector
    mutated = Document(
        '<main><section><div><span title="same">decoy</span></div></section>'
        '<span title="same">inserted</span><section data-testid="chosen">'
        '<article><div><span title="same">target</span></div></article>'
        "</section></main>"
    )
    matches = mutated.xpath(selector)
    assert len(matches) == 1 and matches[0].text == "target"


def test_xpath_ambiguous_witness_must_have_same_sibling_position():
    doc = Document(
        '<main><div><span title="same">decoy</span>'
        '<span title="same">target</span></div></main>'
    )
    assert assert_target(doc, doc.select("span")[1]) == "//span[2]"
