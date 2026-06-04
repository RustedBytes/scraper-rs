use criterion::{BenchmarkId, Criterion, SamplingMode, criterion_group, criterion_main};
use scraper_rs::Document;

mod common;

fn bench_parse_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_scaling");
    group.sampling_mode(SamplingMode::Flat);

    for size_bytes in common::progressive_sizes() {
        let html = common::progressive_html(size_bytes);
        group.throughput(criterion::Throughput::Bytes(html.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format_bytes(html.len())),
            &html,
            |b, html| {
                b.iter(|| Document::new(html, None, false).expect("document should parse"));
            },
        );
    }
    group.finish();
}

fn format_bytes(bytes: usize) -> String {
    const KIB: usize = 1_024;
    const MIB: usize = 1_024 * KIB;

    if bytes >= MIB {
        format!("{:.2} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

criterion_group!(benches, bench_parse_scaling);
criterion_main!(benches);
