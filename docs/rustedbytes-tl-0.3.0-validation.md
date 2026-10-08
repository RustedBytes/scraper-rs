# rustedbytes-tl 0.3.0 compatibility and performance check

## Scope and correctness

Upgrade only `rustedbytes-tl` from 0.2.0 to the latest crates.io release verified by `cargo search` and `cargo info` during this check, 0.3.0. Keep the `std` feature, public API, and other locked dependency versions unchanged.

Base: `00e01f8698e9f9f1b06b70833fa1860339bb576b` (scraper-rs 0.16.1). Candidate: that source with the dependency manifest/lockfile changes in this PR. No Rust or Python implementation files changed.

- 0.2.0: **76 Python tests passed**.
- 0.3.0: **76 Python tests and 31 Rust unit tests passed**.
- Release abi3 Python extension built and installed successfully on CPython 3.14.7.
- `cargo fmt --all --check` reports pre-existing formatting differences in unchanged `src/selector_generator.rs`.
- `cargo clippy --release --all-targets --locked -- -D warnings` reports the same four warnings on **both versions**: two `collapsible_if` and two `unnecessary_sort_by` warnings in that file. This dependency update does not resolve unrelated formatting/lint issues.

## Measurement conditions and boundaries

Measured on an AMD EPYC 9V74 virtual host, Linux x86_64, rustc 1.99.0 (`b940084d7`, LLVM 23.1.1), Criterion 0.8.2, CPython 3.14.7. Same source, harness, deterministic fixtures, feature set, release profile and allocator on both versions. No custom RUSTFLAGS or allocator. Release profile: opt-level 3, fat LTO, one codegen unit. Benchmark commands use default crate features (abi3), without extension-module. Python wheels enable extension-module through maturin.

Use existing `benchmarks/common.rs`, `parse_scaling.rs`, and `sync_async.rs` unchanged. `parse_scaling` includes Document construction and drop; small/medium/large selector inputs contain 2/100/1000 matching elements. Sync selector benches exclude Document construction via batched setup, but consume/drop the Document in the measured closure; returned-element destruction follows Criterion's batched timing boundary. These are not pure selector traversal timings. Async benches include Document creation, selection and Tokio spawn_blocking dispatch, exclude the input-clone setup, and do not measure the public Python asyncio wrappers. XPath includes the first evaluation on each newly created Document. No network I/O. The throughput labels on sync selector benches use HTML bytes, not bytes newly parsed during the timed operation.

Each case uses 100 samples, default 3-second warm-up and a 5-second target measurement window (Criterion extends windows when necessary). The selective reverse-order repeat uses a 10-second target window and the same 100 samples. Confidence intervals are 95%; every interval is listed as **lower bound, estimate, upper bound**. Positive time changes mean slower. These are operation-time estimates, not request p95/p99 latency.

Runs were sequential. This is a shared virtual host, so external CPU and memory load cannot be controlled. Build artifacts were moved to tmpfs after the disk filled; measurements exclude compilation. Results showing changes must be assessed against the reverse-order repeat, not treated as stable release-level speedups/regressions.

## First pass: 0.2.0, then 0.3.0

| Scenario | 0.2.0 time: 95% CI, estimate | 0.3.0 time: 95% CI, estimate | Criterion time change: 95% CI, estimate |
|---|---|---|---|
| `parse_scaling/2.00 KiB` | 11.005 µs 11.051 µs 11.106 µs | 11.002 µs 11.057 µs 11.118 µs | −0.6399% +0.0497% +0.7855% |
| `parse_scaling/8.00 KiB` | 58.225 µs 58.524 µs 58.839 µs | 58.511 µs 58.876 µs 59.343 µs | −0.2770% +0.6010% +1.6341% |
| `parse_scaling/32.00 KiB` | 183.54 µs 185.54 µs 187.82 µs | 193.62 µs 194.39 µs 195.22 µs | +3.4177% +4.7698% +5.9724% |
| `parse_scaling/128.00 KiB` | 700.00 µs 704.56 µs 709.30 µs | 768.27 µs 782.07 µs 797.95 µs | +8.9351% +11.001% +13.539% |
| `parse_scaling/512.00 KiB` | 19.067 ms 19.396 ms 19.842 ms | 19.553 ms 19.781 ms 20.040 ms | −0.6234% +1.9821% +4.3382% |
| `parse_scaling/2.00 MiB` | 78.804 ms 79.615 ms 80.580 ms | 80.806 ms 81.584 ms 82.574 ms | +0.8716% +2.4742% +4.0143% |
| `parse_scaling/8.00 MiB` | 302.84 ms 305.10 ms 307.62 ms | 307.16 ms 309.55 ms 312.27 ms | +0.3142% +1.4609% +2.5798% |
| `sync_async/sync_select/small` | 1.0903 µs 1.1599 µs 1.2262 µs | 1.0713 µs 1.1103 µs 1.1436 µs | −9.1284% −3.0299% +3.2636% |
| `sync_async/sync_select_first/small` | 976.62 ns 1.0206 µs 1.0576 µs | 962.35 ns 996.15 ns 1.0256 µs | −6.4414% +0.3597% +7.4952% |
| `sync_async/sync_first/small` | 920.91 ns 978.37 ns 1.0351 µs | 912.59 ns 970.17 ns 1.0259 µs | −6.9521% +177.98% +554.37% |
| `sync_async/sync_xpath/small` | 37.262 µs 37.632 µs 38.077 µs | 37.037 µs 37.171 µs 37.315 µs | −0.9294% +0.6688% +2.6884% |
| `sync_async/sync_xpath_first/small` | 34.404 µs 34.754 µs 35.324 µs | 35.586 µs 35.830 µs 36.090 µs | +1.2624% +2.2438% +3.0951% |
| `sync_async/async_spawn_blocking_select/small` | 19.628 µs 20.054 µs 20.478 µs | 22.084 µs 22.826 µs 23.562 µs | +2.6026% +8.5111% +14.373% |
| `sync_async/async_spawn_blocking_xpath/small` | 63.723 µs 65.537 µs 67.247 µs | 84.362 µs 85.363 µs 86.367 µs | +15.049% +20.281% +25.330% |
| `sync_async/sync_select/medium` | 22.080 µs 23.578 µs 25.432 µs | 27.218 µs 28.642 µs 30.292 µs | +11.085% +21.274% +31.811% |
| `sync_async/sync_select_first/medium` | 6.8311 µs 7.2189 µs 7.5483 µs | 9.1973 µs 9.8671 µs 10.533 µs | +15.891% +32.446% +49.492% |
| `sync_async/sync_first/medium` | 7.6285 µs 8.1406 µs 8.5784 µs | 7.6444 µs 7.9235 µs 8.1589 µs | −7.3443% +4.0803% +18.656% |
| `sync_async/sync_xpath/medium` | 1.2198 ms 1.2282 ms 1.2377 ms | 1.1814 ms 1.1940 ms 1.2092 ms | −4.0301% −1.6512% +0.8008% |
| `sync_async/sync_xpath_first/medium` | 1.0086 ms 1.0119 ms 1.0159 ms | 1.0105 ms 1.0238 ms 1.0409 ms | +0.7112% +1.9379% +3.4611% |
| `sync_async/async_spawn_blocking_select/medium` | 83.691 µs 86.297 µs 89.017 µs | 83.027 µs 84.966 µs 86.805 µs | −22.831% +2.2327% +38.145% |
| `sync_async/async_spawn_blocking_xpath/medium` | 1.2914 ms 1.3093 ms 1.3292 ms | 1.3504 ms 1.3659 ms 1.3830 ms | −1.0968% +1.2198% +3.6118% |
| `sync_async/sync_select/large` | 362.81 µs 375.14 µs 388.47 µs | 262.60 µs 267.25 µs 272.88 µs | −31.491% −28.758% −25.861% |
| `sync_async/sync_select_first/large` | 156.45 µs 160.70 µs 164.82 µs | 144.39 µs 147.77 µs 150.80 µs | −16.829% −7.5207% +3.6232% |
| `sync_async/sync_first/large` | 158.25 µs 166.13 µs 174.31 µs | 140.38 µs 143.87 µs 146.91 µs | −17.038% −7.6251% +2.2774% |
| `sync_async/sync_xpath/large` | 15.698 ms 15.835 ms 16.019 ms | 15.738 ms 15.983 ms 16.327 ms | −1.0556% +0.9295% +3.1818% |
| `sync_async/sync_xpath_first/large` | 13.664 ms 13.815 ms 13.997 ms | 13.676 ms 13.842 ms 14.086 ms | −1.6666% +0.2004% +2.2453% |
| `sync_async/async_spawn_blocking_select/large` | 798.08 µs 809.00 µs 821.05 µs | 778.74 µs 790.87 µs 806.64 µs | −13.876% −6.3761% +0.0887% |
| `sync_async/async_spawn_blocking_xpath/large` | 18.216 ms 18.427 ms 18.657 ms | 17.921 ms 18.318 ms 18.787 ms | −2.9748% −0.5901% +1.9748% |

## Reverse-order repeat: 0.3.0, then 0.2.0

Criterion compares the **old 0.2.0** run to the saved **new 0.3.0** baseline in this repeat; the final column therefore has the inverse direction of the first-pass column.

| Scenario | 0.3.0 time: 95% CI, estimate | 0.2.0 time: 95% CI, estimate | 0.2.0 relative to 0.3.0 |
|---|---|---|---|
| `parse_scaling/32.00 KiB` | 165.76 µs 166.40 µs 167.07 µs | 166.02 µs 167.34 µs 168.96 µs | −0.3975% +0.5664% +1.6093% |
| `parse_scaling/128.00 KiB` | 710.45 µs 713.50 µs 716.88 µs | 732.95 µs 743.51 µs 755.96 µs | +2.6265% +4.2059% +6.0265% |
| `sync_async/sync_select/large` | 496.19 µs 526.57 µs 554.00 µs | 423.10 µs 439.94 µs 454.40 µs | −20.010% −12.984% −5.1185% |

## Interpretation

Correctness checks pass, but this host does **not** provide reliable evidence of a dependency-driven speedup or slowdown. The first pass showed +4.77%/+11.00% time changes for 32/128 KiB parsing; the reverse-order repeat changed direction. Large CSS selection likewise flipped from an apparent candidate improvement to an apparent candidate regression. These conflicting results indicate substantial run/environment sensitivity. The remaining cases were measured once and are exploratory results, not confirmed changes. Do not use this report to promise a speedup or rule out regressions on controlled hardware.

The dependency update is API-compatible with this project's test suite. For a performance-sensitive release, rerun on a dedicated idle host and include representative production HTML; these fixtures are synthetic. The existing 0.2.0 parsing times themselves rise sharply between 128 and 512 KiB, so that scaling behavior predates this update.

## Reproduce

Create a virtualenv with Python, maturin, pytest and pytest-asyncio. Set `PYO3_PYTHON` to its Python executable (a usable libpython is needed for Rust tests and benches).

```sh
export PYO3_PYTHON="$PWD/.venv/bin/python"
# On the base revision, with its original manifest and Cargo.lock:
cargo bench --locked --bench sync_async -- --save-baseline tl-020
cargo bench --locked --bench parse_scaling -- --save-baseline tl-020
# On the candidate, keeping target/criterion from the base runs:
cargo bench --locked --bench parse_scaling -- --baseline tl-020
cargo bench --locked --bench sync_async -- --baseline tl-020
cargo test --release --lib --locked
.venv/bin/maturin develop --release --locked
.venv/bin/pytest tests/ -q
# Save candidate controls, then rerun the same commands on 0.2.0
# with --baseline tl-030-repeat in place of --save-baseline:
cargo bench --locked --bench parse_scaling -- 'parse_scaling/(32|128)\.' --save-baseline tl-030-repeat --measurement-time 10
cargo bench --locked --bench sync_async -- 'sync_async/sync_select/large' --save-baseline tl-030-repeat --measurement-time 10
```

Use the exact base lockfile when reproducing 0.2.0. A named Criterion reference baseline is preserved; raw generated target reports are not committed.
