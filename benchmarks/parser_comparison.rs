use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use scraper::{Html, Selector};
use scraper_rs::Document;

mod common;

fn bench_parser_comparison(c: &mut Criterion) {
    let documents = [
        ("small", common::small_html()),
        ("medium", common::medium_html()),
        ("large", common::large_html()),
    ];
    let selector = Selector::parse(common::CSS_ITEM).expect("CSS selector should parse");

    let mut group = c.benchmark_group("parser_comparison");
    for (name, html) in documents {
        group.throughput(criterion::Throughput::Bytes(html.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("scraper_rs_parse", name),
            &html,
            |b, html| {
                b.iter(|| Document::new(html, None, false).expect("document should parse"));
            },
        );

        group.bench_with_input(BenchmarkId::new("scraper_parse", name), &html, |b, html| {
            b.iter(|| Html::parse_document(html));
        });

        group.bench_with_input(
            BenchmarkId::new("scraper_rs_css_select", name),
            &html,
            |b, html| {
                b.iter_batched(
                    || Document::new(html, None, false).expect("document should parse"),
                    |doc| {
                        doc.select(common::CSS_ITEM)
                            .expect("CSS selector should evaluate")
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("scraper_css_select", name),
            &html,
            |b, html| {
                b.iter_batched(
                    || Html::parse_document(html),
                    |doc| doc.select(&selector).count(),
                    BatchSize::SmallInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("scraper_rs_css_select_first", name),
            &html,
            |b, html| {
                b.iter_batched(
                    || Document::new(html, None, false).expect("document should parse"),
                    |doc| {
                        doc.select_first(common::CSS_ITEM)
                            .expect("CSS selector should evaluate")
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("scraper_css_select_first", name),
            &html,
            |b, html| {
                b.iter_batched(
                    || Html::parse_document(html),
                    |doc| doc.select(&selector).next().is_some(),
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_parser_comparison);
criterion_main!(benches);
