# Benchmarks

This directory contains benchmark scripts for measuring the performance of scraper-rs.

## Running Benchmarks

### Prerequisites

Build the package in release mode:

```bash
maturin develop --release
```

### Run the sync vs async benchmark

```bash
python benchmarks/bench_sync_async.py
```

## Benchmark Scripts

### bench_sync_async.py

Compares the performance of synchronous vs asynchronous functions:

- **Synchronous functions**: `select`, `select_first`, `first`, `xpath`, `xpath_first`
- **Asynchronous functions**: `async select`, `async select_first`, `async first`, `async xpath`, `async xpath_first`

Tests are run against three HTML document sizes:
- **Small**: ~200 bytes, 2 items
- **Medium**: ~5KB, 100 items
- **Large**: ~50KB, 1000 items

The benchmark also tests concurrent execution of async functions to demonstrate their value in concurrent scenarios.

## CI Integration

Benchmarks run automatically on pull requests that modify:
- Rust source code (`src/**`)
- Dependencies (`Cargo.toml`)
- Benchmark scripts (`benchmarks/**`)
- The benchmark workflow (`.github/workflows/benchmark.yml`)

You can also trigger benchmarks manually via GitHub Actions workflow dispatch.

## Interpreting Results

- **Sync functions**: Best for sequential, CPU-bound operations
- **Async functions (sequential)**: Similar to sync with slight overhead for context switching
- **Async functions (concurrent)**: Show significant speedup when running multiple operations simultaneously

Note that for CPU-bound operations like HTML parsing, synchronous functions may be faster for sequential execution. However, async functions enable better responsiveness in I/O-bound applications and allow concurrent operations without blocking.
