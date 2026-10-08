#[path = "../benchmarks/common.rs"]
mod common;
#[test]
fn fixtures_have_expected_matches() {
    for (_, html, matches) in common::selection_fixtures() {
        common::validate_fixture(&html, matches);
    }
}
#[test]
fn progressive_fixtures_have_exact_sizes_and_valid_matches() {
    for size in common::progressive_sizes() {
        let html = common::progressive_html(size);
        assert_eq!(html.len(), size);
        let expected = html.matches("<article class='item'").count();
        common::validate_fixture(&html, expected);
    }
}
