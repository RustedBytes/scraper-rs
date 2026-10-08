use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use scraper_rs::Document;
use std::hint::black_box;
mod common;

fn boundaries(c: &mut Criterion) {
    for (name, html, matches) in common::selection_fixtures() {
        common::validate_fixture(&html, matches);
        let mut parse = c.benchmark_group("html_parse");
        parse.throughput(Throughput::Bytes(html.len() as u64));
        parse.bench_with_input(
            BenchmarkId::new("tl_dom_create_no_drop", name),
            &html,
            |b, html| {
                b.iter_batched(
                    || (),
                    |_| tl::parse(black_box(html), Default::default()).unwrap(),
                    BatchSize::PerIteration,
                );
            },
        );
        parse.bench_with_input(
            BenchmarkId::new("document_create_no_drop", name),
            &html,
            |b, html| {
                b.iter_batched(
                    || (),
                    |_| Document::new(black_box(html), None, false).unwrap(),
                    BatchSize::PerIteration,
                );
            },
        );
        parse.bench_with_input(
            BenchmarkId::new("document_create_and_drop", name),
            &html,
            |b, html| {
                b.iter(|| black_box(Document::new(black_box(html), None, false).unwrap()));
            },
        );
        parse.finish();
        let mut drop_group = c.benchmark_group("dom_drop");
        drop_group.throughput(Throughput::Elements(1));
        drop_group.bench_function(BenchmarkId::new("document", name), |b| {
            b.iter_batched(
                || Document::new(&html, None, false).unwrap(),
                |doc| drop(black_box(doc)),
                BatchSize::PerIteration,
            );
        });
        drop_group.bench_function(BenchmarkId::new("tl_dom", name), |b| {
            b.iter_batched(
                || tl::parse(&html, Default::default()).unwrap(),
                |dom| drop(black_box(dom)),
                BatchSize::PerIteration,
            );
        });
        drop_group.finish();
        let doc = Document::new(&html, None, false).unwrap();
        // Warm selector cache; output checks happen outside the measurement.
        assert_eq!(doc.select(common::CSS_ITEM).unwrap().len(), matches);
        let mut css = c.benchmark_group("css_existing_document");
        css.throughput(Throughput::Elements(1));
        css.bench_function(BenchmarkId::new("select_with_result_drop", name), |b| {
            b.iter(|| black_box(doc.select(black_box(common::CSS_ITEM)).unwrap()));
        });
        css.bench_function(
            BenchmarkId::new("select_first_with_result_drop", name),
            |b| {
                b.iter(|| black_box(doc.select_first(black_box(common::CSS_ITEM)).unwrap()));
            },
        );
        css.finish();
    }
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .build()
        .unwrap();
    rt.block_on(async { tokio::task::spawn_blocking(|| ()).await.unwrap() });
    let mut group = c.benchmark_group("tokio_dispatch");
    group.throughput(Throughput::Elements(1));
    group.bench_function("inline_ready", |b| {
        b.to_async(&rt).iter(|| async { black_box(1_u64) })
    });
    group.bench_function("spawn_blocking_roundtrip", |b| {
        b.to_async(&rt).iter(|| async {
            tokio::task::spawn_blocking(|| black_box(1_u64))
                .await
                .unwrap()
        })
    });
    group.finish();
}
criterion_group!(benches, boundaries);
criterion_main!(benches);
