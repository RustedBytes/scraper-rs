# Benchmarks

This directory contains Rust Criterion benchmarks for measuring scraper-rs core performance without Python interpreter overhead.

## Running Benchmarks

Run the full benchmark suite:

```shell
cargo bench
```

Run one benchmark target:

```shell
cargo bench --bench sync_async
cargo bench --bench parser_comparison
cargo bench --bench parse_scaling
```

Criterion writes HTML reports under `target/criterion/`.

## Benchmark Targets

### sync_async.rs

Compares direct synchronous operations with a Tokio `spawn_blocking` path that mirrors the scheduling shape used by the Rust async extension helpers.

Operations benchmarked:

- `select`
- `select_first`
- `find`
- `xpath`
- `xpath_first`
- `spawn_blocking` CSS selection
- `spawn_blocking` XPath selection

Inputs are small, medium, and large deterministic HTML documents.

### parser_comparison.rs

Compares scraper-rs owned `Document`/`Element` behavior with the underlying Rust `scraper` crate baseline.

Operations benchmarked:

- document parse
- CSS select
- CSS select first

The old Python benchmark compared against `markupever`; that library is Python-only, so it is not part of the Rust Criterion suite.

### parse_scaling.rs

Measures parse throughput across progressively larger deterministic HTML inputs:

- 2 KiB
- 8 KiB
- 32 KiB
- 128 KiB
- 512 KiB
- 2 MiB
- 8 MiB

Criterion does not measure RSS directly. This target replaces the old Python memory benchmark with stable parse-throughput scaling over the same style of generated documents.

## Interpreting Results

Use `parser_comparison` to understand wrapper overhead relative to the raw Rust parser. Use `sync_async` to estimate scheduling cost when work is routed through a blocking async task. Use `parse_scaling` to watch parser throughput as input size grows.
