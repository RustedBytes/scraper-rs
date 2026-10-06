# Benchmarks

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

### parse_scaling.rs

Measures parse throughput across progressively larger deterministic HTML inputs:

- 2 KiB
- 8 KiB
- 32 KiB
- 128 KiB
- 512 KiB
- 2 MiB
- 8 MiB

## Interpreting Results

- Use `sync_async` to estimate scheduling cost when work is routed through a blocking async task. 
- Use `parse_scaling` to watch parser throughput as input size grows.

### bench_selectors.py

After `maturin develop --release --locked`, run:

```shell
python benchmarks/bench_selectors.py --repeats 5 --output benchmarks/selectors.json
```

Measures CSS and XPath generation on mirrored deep DOMs (16/64/128 levels),
attribute-heavy DOMs (8/32/128 attributes), and a combined 64-level/16-attribute
case. Reports first-call and repeated-call latency, excluding HTML parsing and
CSS target lookup. XPath result uniqueness and target text are checked outside
the timed region. CSS `None` is allowed for indistinguishable sibling branches.
The XPath compilation cache is thread-local and bounded, so repeated calls can
still compile candidates when the search exceeds cache capacity. CI publishes
JSON rather than enforcing unstable wall-clock thresholds.
