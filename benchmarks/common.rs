#![allow(dead_code)]

use std::fmt::Write;

pub const CSS_ITEM: &str = ".item";
pub const XPATH_ITEM: &str = "//div[@class='item']";

pub fn small_html() -> String {
    r#"
<html>
  <body>
    <div class="item" data-id="1"><a href="/a">First</a></div>
    <div class="item" data-id="2"><a href="/b">Second</a></div>
  </body>
</html>
"#
    .to_string()
}

pub fn medium_html() -> String {
    html_with_items("Test Page", 100, false)
}

pub fn large_html() -> String {
    html_with_items("Large Test Page", 1_000, true)
}

pub fn progressive_html(target_bytes: usize) -> String {
    let head = concat!(
        "<!doctype html><html><head><meta charset='utf-8'>",
        "<title>scraper-rs memory benchmark</title></head><body><main>"
    );
    let tail = "</main></body></html>";
    let static_bytes = head.len() + tail.len();
    if target_bytes <= static_bytes {
        return format!("{head}{tail}");
    }

    let mut html = String::with_capacity(target_bytes + tail.len());
    html.push_str(head);
    let mut total_bytes = static_bytes;
    let mut id = 0;

    loop {
        let article = format!(
            "<article class='item' data-id='{id}'>\
             <h2>Item {id}</h2>\
             <p>Lorem ipsum dolor sit amet, consectetur adipiscing elit.</p>\
             <ul><li>alpha</li><li>beta</li><li>gamma</li></ul>\
             </article>"
        );
        if total_bytes + article.len() > target_bytes {
            break;
        }
        html.push_str(&article);
        total_bytes += article.len();
        id += 1;
    }

    let remaining = target_bytes.saturating_sub(total_bytes);
    if remaining >= 7 {
        html.push_str("<!--");
        html.push_str(&"x".repeat(remaining - 7));
        html.push_str("-->");
    } else if remaining > 0 {
        html.push_str(&"x".repeat(remaining));
    }
    html.push_str(tail);
    html
}

pub fn progressive_sizes() -> Vec<usize> {
    let kib = 1_024;
    let mib = 1_024 * kib;
    vec![
        2 * kib,
        8 * kib,
        32 * kib,
        128 * kib,
        512 * kib,
        2 * mib,
        8 * mib,
    ]
}

fn html_with_items(title: &str, count: usize, include_description: bool) -> String {
    let mut items = String::new();
    for i in 0..count {
        if include_description {
            writeln!(
                items,
                r#"<div class="item" data-id="{i}"><a href="/item{i}">Item {i}</a><p>Description for item {i}</p></div>"#
            )
            .expect("writing to String should not fail");
        } else {
            writeln!(
                items,
                r#"<div class="item" data-id="{i}"><a href="/item{i}">Item {i}</a></div>"#
            )
            .expect("writing to String should not fail");
        }
    }

    format!(
        r#"
<html>
  <head><title>{title}</title></head>
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
"#
    )
}
