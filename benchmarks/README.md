# Benchmarks

This directory contains benchmark scripts for measuring the performance of scraper-rs.

## Running Benchmarks

### Install Python 3.14t

```shell
uv venv --python 3.14t

uv run python -m ensurepip --upgrade

# check Python version
uv run python -VV # should be "Python 3.14.2 free-threading ..."
```

### Prerequisites

Build the package in release mode:

```shell
uv run maturin develop --release --locked
```

For markupever comparison, also install markupever:

```shell
uv pip install markupever
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

The benchmark also tests `asyncio.gather` scheduling overhead for the async wrappers. The current Python wrappers are awaitable for coroutine API consistency, but they do not make CPU-bound parsing parallel on their own.

### bench_vs_markupever.py

Compares scraper-rs performance against [markupever](https://github.com/awolverp/markupever), another Python HTML parsing library based on html5ever.

Operations benchmarked:
- **parse**: Document parsing
- **css_select**: CSS selection with `.select()`
- **css_select_first**: First match with `.select_first()` or `.select_one()`

Tests are run against three HTML document sizes:
- **Small**: ~200 bytes, 2 items
- **Medium**: ~5KB, 100 items
- **Large**: ~50KB, 1000 items

The benchmark shows the ratio between scraper-rs and markupever for each operation. Lower ratios indicate better relative performance.

### bench_parse_memory.py

Measures parser memory usage with progressively larger HTML documents.

- Runs each HTML size in an isolated subprocess (prevents memory carry-over between sizes)
- Uses progressive size steps (default: 2 KiB to 8 MiB, doubling each step)
- Reports per-size parse latency (`median`/`p95`) and RSS memory (`baseline`, `peak`, `delta`)
- Scales iteration count by input size to keep total parsed volume realistic

Run with defaults:

```shell
uv run benchmarks/bench_parse_memory.py
```

Run with custom progression:

```shell
uv run benchmarks/bench_parse_memory.py --min-kib 4 --max-kib 16384 --growth-factor 2
```

Run with explicit size steps:

```shell
uv run benchmarks/bench_parse_memory.py --sizes 4k,16k,64k,256k,1m,4m
```

## Interpreting Results

- **Sync functions**: Best for sequential, CPU-bound operations
- **Async functions (sequential)**: Similar to sync with slight overhead for context switching
- **Async functions (concurrent)**: Measure coroutine scheduling overhead; do not assume CPU-bound parsing runs in parallel

Note that for CPU-bound operations like HTML parsing, synchronous functions are generally the clearest baseline. The async wrappers are most useful when integrating scraper-rs into coroutine-oriented application code.

## Test run

### System

```
Architecture:                x86_64
  CPU op-mode(s):            32-bit, 64-bit
  Address sizes:             48 bits physical, 48 bits virtual
  Byte Order:                Little Endian
CPU(s):                      24
  On-line CPU(s) list:       0-23
Vendor ID:                   AuthenticAMD
  Model name:                AMD Ryzen 9 9900X 12-Core Processor
    CPU family:              26
    Model:                   68
    Thread(s) per core:      2
    Core(s) per socket:      12
    Socket(s):               1
    Stepping:                0
    CPU(s) scaling MHz:      47%
    CPU max MHz:             5658.0000
    CPU min MHz:             600.0000
    BogoMIPS:                8782.96
    Flags:                   fpu vme de pse tsc msr pae mce cx8 apic sep mtrr pge mca cmov pat pse36 clflush mmx fxsr sse sse2 ht s
                             yscall nx mmxext fxsr_opt pdpe1gb rdtscp lm constant_tsc rep_good amd_lbr_v2 nopl nonstop_tsc cpuid ex
                             td_apicid aperfmperf rapl pni pclmulqdq monitor ssse3 fma cx16 sse4_1 sse4_2 movbe popcnt aes xsave av
                             x f16c rdrand lahf_lm cmp_legacy svm extapic cr8_legacy abm sse4a misalignsse 3dnowprefetch osvw ibs s
                             kinit wdt tce topoext perfctr_core perfctr_nb bpext perfctr_llc mwaitx cpb cat_l3 cdp_l3 hw_pstate ssb
                             d mba perfmon_v2 ibrs ibpb stibp ibrs_enhanced vmmcall fsgsbase tsc_adjust bmi1 avx2 smep bmi2 erms in
                             vpcid cqm rdt_a avx512f avx512dq rdseed adx smap avx512ifma clflushopt clwb avx512cd sha_ni avx512bw a
                             vx512vl xsaveopt xsavec xgetbv1 xsaves cqm_llc cqm_occup_llc cqm_mbm_total cqm_mbm_local user_shstk av
                             x_vnni avx512_bf16 clzero irperf xsaveerptr rdpru wbnoinvd cppc amd_ibpb_ret arat npt lbrv svm_lock nr
                             ip_save tsc_scale vmcb_clean flushbyasid decodeassists pausefilter pfthreshold avic v_vmsave_vmload vg
                             if x2avic v_spec_ctrl vnmi avx512vbmi umip pku ospke avx512_vbmi2 gfni vaes vpclmulqdq avx512_vnni avx
                             512_bitalg avx512_vpopcntdq rdpid bus_lock_detect movdiri movdir64b overflow_recov succor smca fsrm av
                             x512_vp2intersect flush_l1d
Virtualization features:     
  Virtualization:            AMD-V
Caches (sum of all):         
  L1d:                       576 KiB (12 instances)
  L1i:                       384 KiB (12 instances)
  L2:                        12 MiB (12 instances)
  L3:                        64 MiB (2 instances)
NUMA:                        
  NUMA node(s):              1
  NUMA node0 CPU(s):         0-23
Vulnerabilities:             
  Gather data sampling:      Not affected
  Indirect target selection: Not affected
  Itlb multihit:             Not affected
  L1tf:                      Not affected
  Mds:                       Not affected
  Meltdown:                  Not affected
  Mmio stale data:           Not affected
  Reg file data sampling:    Not affected
  Retbleed:                  Not affected
  Spec rstack overflow:      Not affected
  Spec store bypass:         Mitigation; Speculative Store Bypass disabled via prctl
  Spectre v1:                Mitigation; usercopy/swapgs barriers and __user pointer sanitization
  Spectre v2:                Mitigation; Enhanced / Automatic IBRS; IBPB conditional; STIBP always-on; PBRSB-eIBRS Not affected; BH
                             I Not affected
  Srbds:                     Not affected
  Tsa:                       Not affected
  Tsx async abort:           Not affected
  Vmscape:                   Not affected
```

### Results (enabled GIL)

Command:

```shell
uv run benchmarks/bench_sync_async.py
```

Output:

```
================================================================================
Scraper-rs Benchmark: Sync vs Async Performance
================================================================================

SMALL HTML (~200 bytes)
--------------------------------------------------------------------------------
Synchronous functions:
  select                        :    444.58 µs total,      4.45 µs avg
  select_first                  :    347.40 µs total,      3.47 µs avg
  first                         :    341.36 µs total,      3.41 µs avg
  xpath                         :      1.08 ms total,     10.77 µs avg
  xpath_first                   :    940.13 µs total,      9.40 µs avg

Asynchronous functions (sequential):
  async select                  :     14.00 ms total,    139.99 µs avg
  async select_first            :     11.15 ms total,    111.51 µs avg
  async first                   :     13.48 ms total,    134.79 µs avg
  async xpath                   :     13.07 ms total,    130.73 µs avg
  async xpath_first             :      9.96 ms total,     99.61 µs avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :    620.41 µs total,     62.04 µs avg
  concurrent xpath              :    219.07 µs total,     21.91 µs avg

MEDIUM HTML (~5KB, 100 items)
--------------------------------------------------------------------------------
Synchronous functions:
  select                        :     14.55 ms total,    145.48 µs avg
  xpath                         :     28.91 ms total,    289.14 µs avg

Asynchronous functions (sequential):
  async select                  :     32.84 ms total,    328.38 µs avg
  async xpath                   :     50.15 ms total,    501.50 µs avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :    615.94 µs total,     61.59 µs avg

LARGE HTML (~50KB, 1000 items)
--------------------------------------------------------------------------------
Synchronous functions:
  select                        :     87.02 ms total,      1.74 ms avg
  xpath                         :    212.63 ms total,      4.25 ms avg

Asynchronous functions (sequential):
  async select                  :    111.84 ms total,      2.24 ms avg
  async xpath                   :    240.10 ms total,      4.80 ms avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :      6.05 ms total,    605.23 µs avg

================================================================================
Summary
================================================================================

Note: Async wrappers provide an awaitable interface for coroutine-oriented
      code, but CPU-bound parsing is still best compared against the sync
      baseline. Concurrent async timings primarily reflect scheduling overhead.
```


### Results (disabled GIL)

Command:

```shell
PYTHON_GIL=0 uv run benchmarks/bench_sync_async.py
```

Output:

```
================================================================================
Scraper-rs Benchmark: Sync vs Async Performance
================================================================================

SMALL HTML (~200 bytes)
--------------------------------------------------------------------------------
Synchronous functions:
  select                        :    446.76 µs total,      4.47 µs avg
  select_first                  :    339.03 µs total,      3.39 µs avg
  first                         :    341.57 µs total,      3.42 µs avg
  xpath                         :      1.07 ms total,     10.68 µs avg
  xpath_first                   :    942.18 µs total,      9.42 µs avg

Asynchronous functions (sequential):
  async select                  :     12.85 ms total,    128.48 µs avg
  async select_first            :     10.26 ms total,    102.58 µs avg
  async first                   :     10.69 ms total,    106.95 µs avg
  async xpath                   :      9.63 ms total,     96.27 µs avg
  async xpath_first             :      9.26 ms total,     92.62 µs avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :    674.20 µs total,     67.42 µs avg
  concurrent xpath              :    194.26 µs total,     19.43 µs avg

MEDIUM HTML (~5KB, 100 items)
--------------------------------------------------------------------------------
Synchronous functions:
  select                        :     14.50 ms total,    145.04 µs avg
  xpath                         :     28.91 ms total,    289.09 µs avg

Asynchronous functions (sequential):
  async select                  :     37.06 ms total,    370.57 µs avg
  async xpath                   :     52.93 ms total,    529.28 µs avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :    554.48 µs total,     55.45 µs avg

LARGE HTML (~50KB, 1000 items)
--------------------------------------------------------------------------------
Synchronous functions:
  select                        :     86.67 ms total,      1.73 ms avg
  xpath                         :    213.82 ms total,      4.28 ms avg

Asynchronous functions (sequential):
  async select                  :    107.18 ms total,      2.14 ms avg
  async xpath                   :    235.05 ms total,      4.70 ms avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :      5.33 ms total,    532.97 µs avg

================================================================================
Summary
================================================================================

Note: Async wrappers provide an awaitable interface for coroutine-oriented
      code, but CPU-bound parsing is still best compared against the sync
      baseline. Concurrent async timings primarily reflect scheduling overhead.
```

## Running markupever comparison benchmark

Command:

```shell
uv run benchmarks/bench_vs_markupever.py
```

Output:

```
==========================================================================================
scraper-rs vs markupever Benchmark
==========================================================================================

SMALL HTML (~200 bytes)
------------------------------------------------------------------------------------------
scraper-rs:
  parse                         :    382.15 µs total,      3.82 µs avg
  css_select                    :    394.83 µs total,      3.95 µs avg
  css_select_first              :    330.69 µs total,      3.31 µs avg

markupever:
  parse                         :    397.00 µs total,      3.97 µs avg
  css_select                    :    443.59 µs total,      4.44 µs avg
  css_select_first              :    525.08 µs total,      5.25 µs avg

  Operation              scraper-rs           markupever         Ratio (scraper-rs/markupever)
  -------------------------------------------------------------------------------------
  parse                      3.82 µs          3.97 µs       0.96x
  css_select                 3.95 µs          4.44 µs       0.89x
  css_select_first           3.31 µs          5.25 µs       0.63x

MEDIUM HTML (~5KB, 100 items)
------------------------------------------------------------------------------------------
scraper-rs:
  parse                         :      9.84 ms total,     98.44 µs avg
  css_select                    :     13.84 ms total,    138.45 µs avg
  css_select_first              :     10.35 ms total,    103.46 µs avg

markupever:
  parse                         :     10.58 ms total,    105.78 µs avg
  css_select                    :     10.66 ms total,    106.57 µs avg
  css_select_first              :     11.12 ms total,    111.16 µs avg

  Operation              scraper-rs           markupever         Ratio (scraper-rs/markupever)
  -------------------------------------------------------------------------------------
  parse                     98.44 µs        105.78 µs       0.93x
  css_select               138.45 µs        106.57 µs       1.30x
  css_select_first         103.46 µs        111.16 µs       0.93x

LARGE HTML (~50KB, 1000 items)
------------------------------------------------------------------------------------------
scraper-rs:
  parse                         :     56.68 ms total,      1.13 ms avg
  css_select                    :     82.28 ms total,      1.65 ms avg
  css_select_first              :     56.09 ms total,      1.12 ms avg

markupever:
  parse                         :     59.92 ms total,      1.20 ms avg
  css_select                    :     60.07 ms total,      1.20 ms avg
  css_select_first              :     60.23 ms total,      1.20 ms avg

  Operation              scraper-rs           markupever         Ratio (scraper-rs/markupever)
  -------------------------------------------------------------------------------------
  parse                      1.13 ms          1.20 ms       0.95x
  css_select                 1.65 ms          1.20 ms       1.37x
  css_select_first           1.12 ms          1.20 ms       0.93x

==========================================================================================
Summary
==========================================================================================

This benchmark compares scraper-rs (after optimizations) with markupever.
scraper-rs still uses the Rust `scraper` crate for CSS-facing HTML parsing, which may depend on html5ever internally; its parse-tree dictionary helpers are backed by rustedbytes-tl.

Key observations:
- scraper-rs now has lazy XPath parsing (only parsed when needed)
- scraper-rs uses lazy property computation for Element attributes
- Ratios < 2.0x indicate scraper-rs is competitive with markupever
- The 'atomic' feature is enabled for enhanced thread safety
```

## Running memory usage benchmark

Command:

```shell
uv run benchmarks/bench_parse_memory.py
```

Output:

```
==============================================================================================
scraper-rs Parse Memory Benchmark (progressive HTML sizes)
==============================================================================================
Sizes: 2.00 KiB -> 8.00 MiB (growth factor 2)
Iterations per size: scaled to ~64 MiB parsed (min=5, max=120)

[run] target=  2.00 KiB iterations=120 warmup=3
[run] target=  4.00 KiB iterations=120 warmup=3
[run] target=  8.00 KiB iterations=120 warmup=3
[run] target= 16.00 KiB iterations=120 warmup=3
[run] target= 32.00 KiB iterations=120 warmup=3
[run] target= 64.00 KiB iterations=120 warmup=3
[run] target=128.00 KiB iterations=120 warmup=3
[run] target=256.00 KiB iterations=120 warmup=3
[run] target=512.00 KiB iterations=120 warmup=3
[run] target=  1.00 MiB iterations= 64 warmup=3
[run] target=  2.00 MiB iterations= 32 warmup=3
[run] target=  4.00 MiB iterations= 16 warmup=3
[run] target=  8.00 MiB iterations=  8 warmup=3

  Input Size   Runs  Median ms   P95 ms   Base RSS   Peak RSS  Delta RSS Delta/Input
----------------------------------------------------------------------------------------------
    2.00 KiB    120      0.023    0.025      26.38      26.92       0.54      276.00x
    4.00 KiB    120      0.044    0.048      26.42      26.97       0.55      141.00x
    8.00 KiB    120      0.091    0.094      26.51      26.94       0.43       54.50x
   16.00 KiB    120      0.173    0.178      26.65      27.32       0.67       42.75x
   32.00 KiB    120      0.339    0.344      26.79      27.68       0.89       28.62x
   64.00 KiB    120      0.684    0.699      27.28      28.52       1.24       19.81x
  128.00 KiB    120      1.351    1.366      29.19      29.69       0.50        3.97x
  256.00 KiB    120      2.646    2.695      32.07      32.57       0.50        2.00x
  512.00 KiB    120      5.396    5.428      38.15      38.65       0.50        1.00x
    1.00 MiB     64     10.798   10.921      50.57      51.06       0.49        0.49x
    2.00 MiB     32     24.132   24.643      42.19      59.92      17.73        8.87x
    4.00 MiB     16     49.059   50.575      58.02      92.82      34.80        8.70x
    8.00 MiB      8     98.611   99.002      89.16     158.09      68.93        8.62x

Notes:
- Each size runs in a fresh subprocess to keep RSS measurements isolated.
- Delta RSS is peak RSS minus baseline RSS measured around the parse loop.
```
