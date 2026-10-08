# Reproducible benchmark boundaries

The additive `boundaries` target separates operations previously mixed in
`sync_async` and `parse_scaling`. Production APIs and dependencies are unchanged.
Criterion remains locked to 0.8.2. Its `iter_batched` PerIteration implementation
times only the routine, then drops its returned output after stopping the timer.
This bounds live DOMs to one; timer overhead matters for very short operations.

| ID prefix | Timed operation | Excluded work | Unit |
| --- | --- | --- | --- |
| html_parse/tl_dom_create_no_drop | Borrowed HTML → TL DOM | fixture generation, DOM drop | input bytes/s |
| html_parse/document_create_no_drop | Public Rust Document constructor (owned source + DOM + state) | fixture generation, Document drop | input bytes/s |
| html_parse/document_create_and_drop | Same constructor and destruction | fixture generation | input bytes/s |
| dom_drop/tl_dom | Drop a fresh borrowed TL DOM | parsing | DOMs/s |
| dom_drop/document | Drop a fresh Document | construction | documents/s |
| css_existing_document/select_with_result_drop | CSS query + owned element snapshots + result destruction | Document construction/destruction | queries/s |
| css_existing_document/select_first_with_result_drop | First match + snapshot + result destruction | Document construction/destruction | queries/s |
| tokio_dispatch/inline_ready | Ready future executor control | runtime creation | operations/s |
| tokio_dispatch/spawn_blocking_roundtrip | Submission, worker scheduling, execution, join | runtime startup/warmup | roundtrips/s |

HTML parsing itself creates a DOM: there is no separate parser-only public API.
The TL case borrows the fixture; Document includes ownership and bookkeeping.
Do not subtract independently estimated medians to claim an exact ownership or
scheduling cost. CSS uses a persistent document and warmed selector cache;
output snapshot allocation is part of the public operation, not pure traversal.
Tokio uses one async worker and one blocking worker, sequential awaited operations.
This control does not reproduce production contention or the Python executor.

Legacy `sync_async` and `parse_scaling` IDs, fixtures and timing loops remain
available. Legacy selector throughput now means operations/s rather than HTML
bytes/s. Historical byte throughput is not comparable. Legacy sync selection
still includes Document destruction; legacy async cases still include parsing.
`parse_scaling` still includes construction and destruction. Compare wall-time
only across identical boundaries. Existing validation reports are historical;
current Python wrappers delegate to Rust async core.

## Validate and run

Use a Python interpreter with libpython, and a release extension built from this
checkout. Python installation/build commands follow the project's development
instructions. Do not enable `extension-module` for standalone Rust executables.

```sh
export PYO3_PYTHON="$PWD/.venv/bin/python"
cargo test --locked --lib --test benchmark_fixtures
cargo bench --locked --bench boundaries --no-run
cargo bench --locked --bench boundaries -- --test
cargo bench --locked --bench allocations > allocations.jsonl
cargo bench --locked --bench boundaries -- --save-baseline unique-reference
python benchmarks/python_wrappers.py > python-wrappers.json
pytest tests/test_benchmark_tools.py
```

Allocation executable counts successful allocation/reallocation calls,
deallocation calls and cumulative requested bytes on the calling thread only.
Reallocation bytes count the full new request, not net growth. These are not live
bytes, peak RSS or foreign/Python allocator counts. Fixture creation, warmed CSS
cache, reporting and output destruction are excluded unless explicitly named.
Ten independent operations per fixture retain all rows. A known Vec allocation
checks the counter before reporting. Instrumentation forwards to System and is
absent from Criterion timing executables. Async worker allocations are deliberately
not presented as captured by this thread-local counter.

Python measurements use a single long-lived asyncio loop, warmup, GC enabled,
sequential awaits, alternating case order and raw batch means in ns/op. They
measure real public parse/select wrappers alongside extension-level core calls.
`python_wrap_elements_only` directly measures Python object wrapping of previously
created core snapshots. Core and public end-to-end differences also contain noise;
they are not exact wrapper costs. Parse cases include explicit close and result
destruction. Record extension hash, Python version and build provenance. Python
results are descriptive batch means, not Criterion confidence intervals or tails.

## Version comparison

Prepare two isolated checkouts, apply identical benchmark sources and benchmark
manifest entries to both; preserve each dependency lockfile. For TL-only
comparisons use the same production source/compiler/features and change only TL
and necessary lock entries. Comparing unrelated source changes answers a different
question. Never use an already installed wheel from another revision.

```sh
python tools/repeat_benchmarks.py /path/base /path/candidate /path/new-results --repeats 6
```

The runner rejects unequal harnesses, uses isolated build directories, runs
sequential balanced AB/BA pairs, rejects differing compiler versions, and stores benchmark-source snapshots,
immutable named per-run baselines, raw
Criterion reports, exact commands, source diffs, manifests, lockfiles and host
metadata. Output must not already exist. Both revisions are prebuilt before sampling. The default 30-second settling
period (`--settle-seconds`) is configurable and cannot prove thermal stability;
verify the host has returned to a stable state before material comparisons. Use the same CPU affinity and idle
host; document governor, power/thermal state, virtualization and external load.
`--controlled-host` records the operator's assertion; it does not enforce isolation.
Keep reports outside git; they can be large. The runner deliberately does not
compare to Criterion's incidental previous-run baseline or auto-gate regressions.

Inspect each saved `estimates.json` and `sample.json`; retain confidence bounds,
estimate, sample count, run order and run-to-run spread. For paired revisions,
time change is `(candidate/base - 1)*100`; positive means slower. Speedup ratio
is `base/candidate`, and is a different metric. Default Criterion 100 samples,
3 s warmup, 5 s measurement, 95% confidence, 5% significance and 1% noise threshold
are retained. Bootstrap samples are not independent host repetitions. Multiple
cases increase false-positive opportunities; confirm important cases separately
against a predefined practical threshold. Do not combine within-run intervals
as if they covered host drift, cherry-pick runs, or discard outliers silently.

Smoke/test mode is correctness validation only. On shared virtual hardware,
report timings as exploratory and performance direction as inconclusive when
order or repeats disagree. This harness change makes no speedup/regression claim.
Fixtures are deterministic synthetic HTML (2/100/1000 CSS matches); parsing
scaling fixtures have exact requested UTF-8 byte sizes. Tests check sizes and
TL/Document result counts. Add representative production HTML with recorded
provenance before generalizing beyond these inputs.

Primary timing reference: [Criterion 0.8.2 Bencher](https://docs.rs/criterion/0.8.2/criterion/struct.Bencher.html).

## Harness validation on this change

On the available shared Linux virtual host (rustc 1.99.0, CPython 3.14.7):
31 Rust unit tests, 2 fixture integration tests, and all 82 Python tests passed.
Release `boundaries`/`allocations` executables compiled; all 23 new timing cases
and both legacy Criterion targets passed smoke checks. The allocation executable
passed its counter self-check and emitted 150 parse/create/drop/select records.
The public Python wrapper harness completed a short two-repeat smoke run.
One default-sampling Criterion case also generated valid raw JSON: 100 positive
samples and ordered 95% confidence bounds. Its timing is not used as a version
comparison.
Ruff and formatting of changed Rust files pass. Strict full-project Clippy and
`cargo fmt --all --check` still report existing issues in unchanged
`src/selector_generator.rs` (two collapsible-if and two unnecessary-sort-by
warnings, plus formatting); Clippy passes with only those two lint categories
allowed. No production code or dependencies were changed to suppress them.
These checks validate the harness, not a dependency performance improvement.
Dedicated idle hardware and balanced full repetitions remain necessary for a
release-level performance claim.
