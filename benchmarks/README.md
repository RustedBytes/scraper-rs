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

The benchmark also tests concurrent execution of async functions to demonstrate their value in concurrent scenarios.

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

## Interpreting Results

- **Sync functions**: Best for sequential, CPU-bound operations
- **Async functions (sequential)**: Similar to sync with slight overhead for context switching
- **Async functions (concurrent)**: Show significant speedup when running multiple operations simultaneously

Note that for CPU-bound operations like HTML parsing, synchronous functions may be faster for sequential execution. However, async functions enable better responsiveness in I/O-bound applications and allow concurrent operations without blocking.

## Recent Performance Improvements

After optimizations (lazy XPath parsing, lazy property computation, atomic feature):
- scraper-rs is now **1.6-3.4x** faster than before
- scraper-rs is **1.8-3.4x slower** than markupever (down from 9-20x slower)
- The performance gap has been significantly reduced while maintaining full XPath support

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
  select                        :    444.65 µs total,      4.45 µs avg
  select_first                  :    337.87 µs total,      3.38 µs avg
  first                         :    335.80 µs total,      3.36 µs avg
  xpath                         :      1.06 ms total,     10.57 µs avg
  xpath_first                   :    921.52 µs total,      9.22 µs avg

Asynchronous functions (sequential):
  async select                  :     13.68 ms total,    136.82 µs avg
  async select_first            :     10.09 ms total,    100.88 µs avg
  async first                   :      8.50 ms total,     84.95 µs avg
  async xpath                   :     10.51 ms total,    105.07 µs avg
  async xpath_first             :      9.75 ms total,     97.45 µs avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :    690.96 µs total,     69.10 µs avg
  concurrent xpath              :    370.73 µs total,     37.07 µs avg

MEDIUM HTML (~5KB, 100 items)
--------------------------------------------------------------------------------
Synchronous functions:
  select                        :     14.30 ms total,    143.00 µs avg
  xpath                         :     28.92 ms total,    289.23 µs avg

Asynchronous functions (sequential):
  async select                  :     37.90 ms total,    379.00 µs avg
  async xpath                   :     53.10 ms total,    530.99 µs avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :    468.07 µs total,     46.81 µs avg

LARGE HTML (~50KB, 1000 items)
--------------------------------------------------------------------------------
Synchronous functions:
  select                        :     87.94 ms total,      1.76 ms avg
  xpath                         :    213.16 ms total,      4.26 ms avg

Asynchronous functions (sequential):
  async select                  :    109.81 ms total,      2.20 ms avg
  async xpath                   :    242.50 ms total,      4.85 ms avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :      6.02 ms total,    601.54 µs avg

================================================================================
Summary
================================================================================

Note: Async functions show their value in concurrent scenarios where
      multiple operations can be performed simultaneously without blocking.
      For CPU-bound operations like HTML parsing, sync functions may be
      faster for sequential execution, but async allows better responsiveness
      in I/O-bound applications.
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
  select                        :    444.07 µs total,      4.44 µs avg
  select_first                  :    336.63 µs total,      3.37 µs avg
  first                         :    335.54 µs total,      3.36 µs avg
  xpath                         :      1.07 ms total,     10.75 µs avg
  xpath_first                   :    934.05 µs total,      9.34 µs avg

Asynchronous functions (sequential):
  async select                  :     11.98 ms total,    119.84 µs avg
  async select_first            :     11.78 ms total,    117.77 µs avg
  async first                   :     11.95 ms total,    119.52 µs avg
  async xpath                   :     14.76 ms total,    147.60 µs avg
  async xpath_first             :     13.89 ms total,    138.86 µs avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :    754.15 µs total,     75.41 µs avg
  concurrent xpath              :    289.25 µs total,     28.93 µs avg

MEDIUM HTML (~5KB, 100 items)
--------------------------------------------------------------------------------
Synchronous functions:
  select                        :     14.40 ms total,    144.01 µs avg
  xpath                         :     28.75 ms total,    287.53 µs avg

Asynchronous functions (sequential):
  async select                  :     32.47 ms total,    324.69 µs avg
  async xpath                   :     48.13 ms total,    481.31 µs avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :    473.12 µs total,     47.31 µs avg

LARGE HTML (~50KB, 1000 items)
--------------------------------------------------------------------------------
Synchronous functions:
  select                        :     86.93 ms total,      1.74 ms avg
  xpath                         :    211.43 ms total,      4.23 ms avg

Asynchronous functions (sequential):
  async select                  :    106.97 ms total,      2.14 ms avg
  async xpath                   :    240.12 ms total,      4.80 ms avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :      5.94 ms total,    593.69 µs avg

================================================================================
Summary
================================================================================

Note: Async functions show their value in concurrent scenarios where
      multiple operations can be performed simultaneously without blocking.
      For CPU-bound operations like HTML parsing, sync functions may be
      faster for sequential execution, but async allows better responsiveness
      in I/O-bound applications.
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
  parse                         :    375.00 µs total,      3.75 µs avg
  css_select                    :    400.75 µs total,      4.01 µs avg
  css_select_first              :    335.96 µs total,      3.36 µs avg

markupever:
  parse                         :    400.41 µs total,      4.00 µs avg
  css_select                    :    441.63 µs total,      4.42 µs avg
  css_select_first              :    518.41 µs total,      5.18 µs avg

  Operation              scraper-rs           markupever         Ratio (scraper-rs/markupever)
  -------------------------------------------------------------------------------------
  parse                      3.75 µs          4.00 µs       0.94x
  css_select                 4.01 µs          4.42 µs       0.91x
  css_select_first           3.36 µs          5.18 µs       0.65x

MEDIUM HTML (~5KB, 100 items)
------------------------------------------------------------------------------------------
scraper-rs:
  parse                         :      9.65 ms total,     96.51 µs avg
  css_select                    :     13.80 ms total,    138.02 µs avg
  css_select_first              :     10.24 ms total,    102.45 µs avg

markupever:
  parse                         :     10.92 ms total,    109.18 µs avg
  css_select                    :     10.70 ms total,    107.03 µs avg
  css_select_first              :     11.18 ms total,    111.84 µs avg

  Operation              scraper-rs           markupever         Ratio (scraper-rs/markupever)
  -------------------------------------------------------------------------------------
  parse                     96.51 µs        109.18 µs       0.88x
  css_select               138.02 µs        107.03 µs       1.29x
  css_select_first         102.45 µs        111.84 µs       0.92x

LARGE HTML (~50KB, 1000 items)
------------------------------------------------------------------------------------------
scraper-rs:
  parse                         :     56.79 ms total,      1.14 ms avg
  css_select                    :     84.03 ms total,      1.68 ms avg
  css_select_first              :     56.47 ms total,      1.13 ms avg

markupever:
  parse                         :     60.11 ms total,      1.20 ms avg
  css_select                    :     60.27 ms total,      1.21 ms avg
  css_select_first              :     60.42 ms total,      1.21 ms avg

  Operation              scraper-rs           markupever         Ratio (scraper-rs/markupever)
  -------------------------------------------------------------------------------------
  parse                      1.14 ms          1.20 ms       0.94x
  css_select                 1.68 ms          1.21 ms       1.39x
  css_select_first           1.13 ms          1.21 ms       0.93x

==========================================================================================
Summary
==========================================================================================

This benchmark compares scraper-rs (after optimizations) with markupever.
Both libraries are based on html5ever for HTML parsing.

Key observations:
- scraper-rs now has lazy XPath parsing (only parsed when needed)
- scraper-rs uses lazy property computation for Element attributes
- Ratios < 2.0x indicate scraper-rs is competitive with markupever
- The 'atomic' feature is enabled for enhanced thread safety
```
