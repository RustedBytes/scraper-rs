use super::*;
use std::borrow::Cow;
use std::rc::Rc;
use std::sync::Once;

use pyo3::types::PyModule;
use pyo3::{Py, Python};

use crate::cache::FixedCache;
use crate::functions::{first, parse, prettify, select, select_first, xpath, xpath_first};
use crate::limits::ensure_within_size_limit;
use crate::prettify::{escape_html, prettify_document_html, prettify_fragment_html};
use crate::selectors::{select_fragment, select_fragment_first, select_with_limit};
use crate::text::{
    attrs_from_element_html, inner_html_from_element_html, normalize_text_nodes,
    text_from_element_html, truncate_for_repr,
};
use crate::xpath::{
    compile_xpath, evaluate_fragment_xpath, evaluate_fragment_xpath_first, xpath_first_with_limit,
    xpath_with_limit,
};

const SAMPLE_HTML: &str = r#"
        <html>
          <body>
            <div class="item" data-id="1"><a href="/a">First</a></div>
            <div class="item" data-id="2"><a href="/b">Second</a></div>
          </body>
        </html>
    "#;

fn init_python() {
    static INIT: Once = Once::new();
    INIT.call_once(Python::initialize);
}

#[test]
fn fixed_cache_evicts_oldest_entry() {
    let mut cache = FixedCache::new(2);
    cache.insert("first".to_string(), 1_u8);
    cache.insert("second".to_string(), 2_u8);
    cache.insert("third".to_string(), 3_u8);

    assert!(cache.get("first").is_none());
    assert_eq!(cache.get("second"), Some(&2_u8));
    assert_eq!(cache.get("third"), Some(&3_u8));
}

#[test]
fn fixed_cache_overwrites_existing_entry() {
    let mut cache = FixedCache::new(2);
    cache.insert("first".to_string(), 1_u8);
    cache.insert("second".to_string(), 2_u8);
    cache.insert("first".to_string(), 9_u8);

    assert_eq!(cache.get("first"), Some(&9_u8));
    assert_eq!(cache.get("second"), Some(&2_u8));
}

#[test]
fn ensure_within_size_limit_returns_borrowed_when_within_limit() {
    let html = "<div>ok</div>";
    let limited = ensure_within_size_limit(html, html.len(), false).unwrap();
    assert!(matches!(limited, Cow::Borrowed(_)));
}

#[test]
fn ensure_within_size_limit_truncates_on_utf8_boundary() {
    let html = "<p>ab😀cd</p>";
    let emoji_start = html.find('😀').unwrap();
    let limit_inside_emoji = emoji_start + 1;

    let truncated = ensure_within_size_limit(html, limit_inside_emoji, true)
        .unwrap()
        .into_owned();

    assert!(truncated.len() < limit_inside_emoji);
    assert!(truncated.is_char_boundary(truncated.len()));
    assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
}

#[test]
fn ensure_within_size_limit_errors_without_truncate() {
    let html = "<div>too big</div>";
    assert!(ensure_within_size_limit(html, 4, false).is_err());
}

#[test]
fn normalize_text_nodes_collapses_whitespace() {
    let normalized = normalize_text_nodes([" one\t", "\n two  ", "three "]);
    assert_eq!(normalized, "one two three");
}

#[test]
fn extractors_return_expected_element_parts() {
    let element_html = r#"<div id="x" class="item"><span> Hello </span><b>World</b></div>"#;

    assert_eq!(
        inner_html_from_element_html(element_html),
        "<span> Hello </span><b>World</b>"
    );
    assert_eq!(text_from_element_html(element_html), "Hello World");

    let attrs = attrs_from_element_html(element_html);
    assert_eq!(attrs.get("id"), Some(&"x".to_string()));
    assert_eq!(attrs.get("class"), Some(&"item".to_string()));
}

#[test]
fn select_fragment_reports_invalid_css() {
    let message = match select_fragment("<div></div>", "div[") {
        Ok(_) => panic!("expected invalid selector to fail"),
        Err(err) => err.to_string(),
    };
    assert!(message.contains("Invalid CSS selector"));
}

#[test]
fn compile_xpath_reuses_cached_instances() {
    init_python();
    let first = compile_xpath(".//div").unwrap();
    let second = compile_xpath(".//div").unwrap();
    assert!(Rc::ptr_eq(&first, &second));
}

#[test]
fn compile_xpath_reports_invalid_expression() {
    init_python();
    let err = compile_xpath("//*[").unwrap_err();
    let message = err.to_string();
    assert!(message.contains("Invalid XPath"));
}

#[test]
fn select_fragment_helpers_return_expected_matches() {
    let html = r#"<section><a href="/a">A</a><a href="/b">B</a></section>"#;

    let all = select_fragment(html, "a[href]").unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].text(), "A");
    assert_eq!(all[1].attr("href"), Some("/b".to_string()));

    let first = select_fragment_first(html, "a[href]").unwrap().unwrap();
    assert_eq!(first.text(), "A");
}

#[test]
fn fragment_xpath_helpers_return_expected_matches() {
    init_python();
    let html = r#"<ul><li><a>A</a></li><li><a>B</a></li></ul>"#;

    let all = evaluate_fragment_xpath(html, ".//li/a").unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].text(), "A");
    assert_eq!(all[1].text(), "B");

    let missing = evaluate_fragment_xpath_first(html, ".//p").unwrap();
    assert!(missing.is_none());
}

#[test]
fn fragment_xpath_helpers_error_on_non_element_results() {
    init_python();
    let html = r#"<div><a>First</a></div>"#;

    let err_message = match evaluate_fragment_xpath(html, "string(.//a)") {
        Ok(_) => panic!("expected xpath evaluation to fail for non-element result"),
        Err(err) => err.to_string(),
    };
    assert!(err_message.contains("must return element nodes"));

    let first_err_message = match evaluate_fragment_xpath_first(html, "string(.//a)") {
        Ok(_) => panic!("expected xpath_first evaluation to fail for non-element result"),
        Err(err) => err.to_string(),
    };
    assert!(first_err_message.contains("must return element nodes"));
}

#[test]
fn select_and_xpath_with_limit_respect_truncation() {
    init_python();
    let html = concat!(
        r#"<div id="start">begin</div>"#,
        r#"<div id="middle">this content is intentionally long for truncation</div>"#,
        r#"<div id="end">finish</div>"#
    );
    let limit = html.find("id=\"end\"").unwrap();

    let start_css = select_with_limit(html, "#start", Some(limit), true).unwrap();
    let end_css = select_with_limit(html, "#end", Some(limit), true).unwrap();
    assert_eq!(start_css.len(), 1);
    assert!(end_css.is_empty());

    let start_xpath = xpath_with_limit(html, "//*[@id='start']", Some(limit), true);
    assert!(
        start_xpath.is_ok(),
        "xpath should succeed for retained prefix"
    );
    assert_eq!(start_xpath.unwrap_or_default().len(), 1);

    let end_xpath = xpath_with_limit(html, "//*[@id='end']", Some(limit), true);
    assert!(
        end_xpath.is_ok(),
        "xpath should succeed for truncated suffix"
    );
    assert!(end_xpath.unwrap_or_default().is_empty());
}

#[test]
fn xpath_first_with_limit_respects_truncation() {
    init_python();
    let html = concat!(
        r#"<div id="start">begin</div>"#,
        r#"<div id="middle">this content is intentionally long for truncation</div>"#,
        r#"<div id="end">finish</div>"#
    );
    let limit = html.find("id=\"end\"").unwrap();

    let start = xpath_first_with_limit(html, "//*[@id='start']", Some(limit), true)
        .unwrap()
        .expect("expected start element");
    assert_eq!(start.attr("id"), Some("start".to_string()));

    let end = xpath_first_with_limit(html, "//*[@id='end']", Some(limit), true).unwrap();
    assert!(end.is_none());
}

#[test]
fn prettify_helpers_format_document_and_fragment() {
    let doc_pretty =
        prettify_document_html(r#"<div id="x"><span>Hi</span><p>A &amp; B</p><br></div>"#);
    assert!(doc_pretty.contains(r#"<div id="x">"#));
    assert!(doc_pretty.contains("<span>Hi</span>"));
    assert!(doc_pretty.contains("<p>A &amp; B</p>"));
    assert!(doc_pretty.contains("<br>"));

    let fragment_pretty =
        prettify_fragment_html(r#"hello <span data-x="1"> world </span>"#).unwrap();
    assert_eq!(fragment_pretty, "hello\n<span data-x=\"1\">world</span>");
}

#[test]
fn prettify_document_html_returns_empty_for_whitespace() {
    assert_eq!(prettify_document_html(" \n\t  "), "");
}

#[test]
fn escape_html_encodes_special_characters() {
    let escaped = escape_html(r#"5 < 7 & 8 > 3 "quote" 'single'"#);
    assert_eq!(
        escaped,
        "5 &lt; 7 &amp; 8 &gt; 3 &quot;quote&quot; &#39;single&#39;"
    );
}

#[test]
fn document_from_html_constructor_matches_new() {
    init_python();
    let via_new = Document::new(SAMPLE_HTML, None, false).unwrap();
    let via_from_html = Document::from_html(SAMPLE_HTML, None, false).unwrap();

    assert_eq!(via_new.text(), via_from_html.text());
    assert_eq!(via_new.select("a").unwrap().len(), 2);
    assert_eq!(via_from_html.select("a").unwrap().len(), 2);
}

#[test]
fn document_close_clears_state_and_is_idempotent() {
    init_python();
    let mut doc = Document::new(SAMPLE_HTML, None, false).unwrap();

    assert_eq!(doc.select("a").unwrap().len(), 2);
    let initial_xpath = doc.xpath(".//a");
    assert!(initial_xpath.is_ok(), "xpath should succeed before close");
    assert_eq!(initial_xpath.unwrap_or_default().len(), 2);

    doc.close();
    doc.close();

    assert_eq!(doc.html(), "");
    assert_eq!(doc.text(), "");
    assert!(doc.select("a").unwrap().is_empty());
    assert!(doc.select_first("a").unwrap().is_none());
    let closed_xpath = doc.xpath(".//a");
    assert!(closed_xpath.is_ok(), "xpath should succeed after close");
    assert!(closed_xpath.unwrap_or_default().is_empty());
    let closed_xpath_first = doc.xpath_first(".//a");
    assert!(
        closed_xpath_first.is_ok(),
        "xpath_first should succeed after close"
    );
    assert!(closed_xpath_first.unwrap_or(None).is_none());
    assert_eq!(doc.prettify().unwrap(), "");
}

#[test]
fn truncate_for_repr_adds_ellipsis_when_needed() {
    assert_eq!(truncate_for_repr("abcdef", 3), "abc...");
    assert_eq!(truncate_for_repr("abc", 3), "abc");
}

#[test]
fn element_methods_aliases_and_repr_work() {
    init_python();
    let html = r#"<div class="item" data-id="7"><a href="/x"> Link </a></div>"#;
    let element = select_fragment(html, "div.item")
        .unwrap()
        .into_iter()
        .next()
        .expect("expected element");

    assert_eq!(element.tag(), "div");
    assert_eq!(element.text(), "Link");
    assert!(element.html().contains(r#"<a href="/x"> Link </a>"#));

    let attrs = element.attrs();
    assert_eq!(attrs.get("class"), Some(&"item".to_string()));
    assert_eq!(element.attr("data-id"), Some("7".to_string()));
    assert_eq!(
        element.get("missing", Some("fallback".to_string())),
        Some("fallback".to_string())
    );

    assert_eq!(element.select("a").unwrap().len(), 1);
    assert_eq!(element.css("a").unwrap().len(), 1);
    assert!(element.find("a").unwrap().is_some());
    assert_eq!(element.xpath(".//a").unwrap().len(), 1);
    assert!(element.xpath_first(".//p").unwrap().is_none());

    let pretty = element.prettify().unwrap();
    assert!(pretty.contains("<a href=\"/x\">Link</a>"));

    Python::attach(|py| {
        let dict = element.to_dict(py).unwrap();
        let dict = dict.bind(py);
        assert_eq!(
            dict.get_item("tag")
                .unwrap()
                .expect("missing tag")
                .extract::<String>()
                .unwrap(),
            "div"
        );
        assert_eq!(
            dict.get_item("text")
                .unwrap()
                .expect("missing text")
                .extract::<String>()
                .unwrap(),
            "Link"
        );
    });

    assert!(element.__repr__().contains("<Element tag='div'"));
}

#[test]
fn document_aliases_repr_and_context_manager_work() {
    init_python();
    let mut doc = Document::new(SAMPLE_HTML, None, false).unwrap();

    assert_eq!(
        doc.find("a").unwrap().and_then(|el| el.attr("href")),
        Some("/a".to_string())
    );
    assert_eq!(doc.css("a").unwrap().len(), 2);
    assert_eq!(
        doc.__repr__(),
        format!("<Document len_html={}>", SAMPLE_HTML.len())
    );

    Python::attach(|py| {
        let doc_obj = Py::new(py, Document::new(SAMPLE_HTML, None, false).unwrap()).unwrap();
        let bound = doc_obj.bind(py);

        let entered = bound.call_method0("__enter__").unwrap();
        assert!(entered.is(bound));

        bound
            .call_method1("__exit__", (py.None(), py.None(), py.None()))
            .unwrap();

        let closed = doc_obj.borrow(py);
        assert_eq!(closed.html(), "");
        assert!(closed.select("a").unwrap().is_empty());
    });

    doc.close();
}

#[test]
fn top_level_pyfunctions_match_document_behavior() {
    init_python();
    let parsed = parse(SAMPLE_HTML, None, false).unwrap();
    assert_eq!(parsed.select("a").unwrap().len(), 2);

    Python::attach(|py| {
        let pretty = prettify(py, SAMPLE_HTML, None, false).unwrap();
        assert!(pretty.contains("<html>"));
        assert!(pretty.contains("<a href=\"/a\">First</a>"));

        let selected = select(py, SAMPLE_HTML, "a", None, false).unwrap();
        assert_eq!(selected.len(), 2);

        let selected_first = select_first(py, SAMPLE_HTML, "a", None, false)
            .unwrap()
            .expect("expected first selected element");
        assert_eq!(selected_first.attr("href"), Some("/a".to_string()));

        let first_alias = first(py, SAMPLE_HTML, "a", None, false)
            .unwrap()
            .expect("expected first element");
        assert_eq!(first_alias.text(), "First");

        let xpath_all = xpath(py, SAMPLE_HTML, ".//a", None, false).unwrap();
        assert_eq!(xpath_all.len(), 2);

        let xpath_first_match = xpath_first(py, SAMPLE_HTML, ".//a", None, false)
            .unwrap()
            .expect("expected first xpath element");
        assert_eq!(xpath_first_match.attr("href"), Some("/a".to_string()));
    });
}

#[test]
fn pymodule_initializer_registers_public_api() {
    init_python();
    Python::attach(|py| {
        let module = PyModule::new(py, "scraper_rs").unwrap();
        scraper_rs(py, &module).unwrap();

        assert!(module.getattr("Document").is_ok());
        assert!(module.getattr("Element").is_ok());
        assert!(module.getattr("_AsyncDocumentCore").is_ok());
        assert!(module.getattr("_AsyncElementCore").is_ok());
        assert!(module.getattr("__version__").is_ok());

        for name in [
            "parse",
            "prettify",
            "select",
            "select_first",
            "first",
            "xpath",
            "xpath_first",
            "parse_async",
            "prettify_async",
            "select_async",
            "select_first_async",
            "first_async",
            "xpath_async",
            "xpath_first_async",
        ] {
            assert!(module.getattr(name).is_ok(), "missing export {name}");
        }
    });
}
