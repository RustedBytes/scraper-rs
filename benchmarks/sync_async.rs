use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use scraper_rs::Document;
use tokio::runtime::Runtime;

mod common;

fn bench_sync_async(c: &mut Criterion) {
    let rt = Runtime::new().expect("tokio runtime should initialize");
    let documents = [
        ("small", common::small_html()),
        ("medium", common::medium_html()),
        ("large", common::large_html()),
    ];

    let mut group = c.benchmark_group("sync_async");
    for (name, html) in documents {
        group.throughput(criterion::Throughput::Bytes(html.len() as u64));

        group.bench_with_input(BenchmarkId::new("sync_select", name), &html, |b, html| {
            b.iter_batched(
                || Document::new(html, None, false).expect("document should parse"),
                |doc| {
                    doc.select(common::CSS_ITEM)
                        .expect("CSS selector should evaluate")
                },
                BatchSize::SmallInput,
            );
        });

        group.bench_with_input(
            BenchmarkId::new("sync_select_first", name),
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

        group.bench_with_input(BenchmarkId::new("sync_first", name), &html, |b, html| {
            b.iter_batched(
                || Document::new(html, None, false).expect("document should parse"),
                |doc| {
                    doc.find(common::CSS_ITEM)
                        .expect("CSS selector should evaluate")
                },
                BatchSize::SmallInput,
            );
        });

        group.bench_with_input(BenchmarkId::new("sync_xpath", name), &html, |b, html| {
            b.iter_batched(
                || Document::new(html, None, false).expect("document should parse"),
                |doc| {
                    doc.xpath(common::XPATH_ITEM)
                        .expect("XPath expression should evaluate")
                },
                BatchSize::SmallInput,
            );
        });

        group.bench_with_input(
            BenchmarkId::new("sync_xpath_first", name),
            &html,
            |b, html| {
                b.iter_batched(
                    || Document::new(html, None, false).expect("document should parse"),
                    |doc| {
                        doc.xpath_first(common::XPATH_ITEM)
                            .expect("XPath expression should evaluate")
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("async_spawn_blocking_select", name),
            &html,
            |b, html| {
                b.to_async(&rt).iter_batched(
                    || html.clone(),
                    |html| async move {
                        tokio::task::spawn_blocking(move || {
                            let doc =
                                Document::new(&html, None, false).expect("document should parse");
                            doc.select(common::CSS_ITEM)
                                .expect("CSS selector should evaluate")
                        })
                        .await
                        .expect("spawn_blocking task should complete")
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("async_spawn_blocking_xpath", name),
            &html,
            |b, html| {
                b.to_async(&rt).iter_batched(
                    || html.clone(),
                    |html| async move {
                        tokio::task::spawn_blocking(move || {
                            let doc =
                                Document::new(&html, None, false).expect("document should parse");
                            doc.xpath(common::XPATH_ITEM)
                                .expect("XPath expression should evaluate")
                        })
                        .await
                        .expect("spawn_blocking task should complete")
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_sync_async);
criterion_main!(benches);
