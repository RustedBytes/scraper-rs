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

## Test run

### System

```
Architecture:                x86_64
  CPU op-mode(s):            32-bit, 64-bit
  Address sizes:             46 bits physical, 48 bits virtual
  Byte Order:                Little Endian
CPU(s):                      2
  On-line CPU(s) list:       0,1
Vendor ID:                   GenuineIntel
  Model name:                Intel(R) Xeon(R) CPU @ 2.20GHz
    CPU family:              6
    Model:                   79
    Thread(s) per core:      2
    Core(s) per socket:      1
    Socket(s):               1
    Stepping:                0
    BogoMIPS:                4399.99
    Flags:                   fpu vme de pse tsc msr pae mce cx8 apic sep mtrr pge mca cmov pat pse36 clflush mmx fxsr sse sse2 ss ht syscall nx pdp
                             e1gb rdtscp lm constant_tsc rep_good nopl xtopology nonstop_tsc cpuid tsc_known_freq pni pclmulqdq ssse3 fma cx16 pcid
                              sse4_1 sse4_2 x2apic movbe popcnt aes xsave avx f16c rdrand hypervisor lahf_lm abm 3dnowprefetch ssbd ibrs ibpb stibp
                              fsgsbase tsc_adjust bmi1 hle avx2 smep bmi2 erms invpcid rtm rdseed adx smap xsaveopt arat md_clear arch_capabilities
Virtualization features:     
  Hypervisor vendor:         KVM
  Virtualization type:       full
Caches (sum of all):         
  L1d:                       32 KiB (1 instance)
  L1i:                       32 KiB (1 instance)
  L2:                        256 KiB (1 instance)
  L3:                        55 MiB (1 instance)
NUMA:                        
  NUMA node(s):              1
  NUMA node0 CPU(s):         0,1
Vulnerabilities:             
  Gather data sampling:      Not affected
  Indirect target selection: Vulnerable
  Itlb multihit:             Not affected
  L1tf:                      Mitigation; PTE Inversion
  Mds:                       Vulnerable; SMT Host state unknown
  Meltdown:                  Vulnerable
  Mmio stale data:           Vulnerable
  Reg file data sampling:    Not affected
  Retbleed:                  Vulnerable
  Spec rstack overflow:      Not affected
  Spec store bypass:         Vulnerable
  Spectre v1:                Vulnerable: __user pointer sanitization and usercopy barriers only; no swapgs barriers
  Spectre v2:                Vulnerable; IBPB: disabled; STIBP: disabled; PBRSB-eIBRS: Not affected; BHI: Vulnerable
  Srbds:                     Not affected
  Tsa:                       Not affected
  Tsx async abort:           Vulnerable
```

### Results (enabled GIL)

Command:

```
uv run bench_sync_async.py
```

Output:

```
================================================================================
Scraper-rs Benchmark: Sync vs Async Performance
================================================================================

SMALL HTML (~200 bytes)
--------------------------------------------------------------------------------
Synchronous functions:
  select                        :      6.62 ms total,     66.25 µs avg
  select_first                  :      6.58 ms total,     65.79 µs avg
  first                         :      6.56 ms total,     65.64 µs avg
  xpath                         :      9.90 ms total,     99.02 µs avg
  xpath_first                   :      9.46 ms total,     94.58 µs avg

Asynchronous functions (sequential):
  async select                  :     96.54 ms total,    965.43 µs avg
  async select_first            :     94.64 ms total,    946.43 µs avg
  async first                   :     93.80 ms total,    937.99 µs avg
  async xpath                   :    104.12 ms total,      1.04 ms avg
  async xpath_first             :     95.16 ms total,    951.62 µs avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :      4.39 ms total,    439.16 µs avg
  concurrent xpath              :      4.58 ms total,    457.91 µs avg

MEDIUM HTML (~5KB, 100 items)
--------------------------------------------------------------------------------
Synchronous functions:
  select                        :    179.86 ms total,      1.80 ms avg
  xpath                         :    253.26 ms total,      2.53 ms avg

Asynchronous functions (sequential):
  async select                  :    285.01 ms total,      2.85 ms avg
  async xpath                   :    343.27 ms total,      3.43 ms avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :     22.66 ms total,      2.27 ms avg

LARGE HTML (~50KB, 1000 items)
--------------------------------------------------------------------------------
Synchronous functions:
  select                        :       1.33 s total,     26.53 ms avg
  xpath                         :       1.71 s total,     34.26 ms avg

Asynchronous functions (sequential):
  async select                  :       1.45 s total,     28.95 ms avg
  async xpath                   :       2.94 s total,     58.88 ms avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :    412.24 ms total,     41.22 ms avg

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

```
PYTHON_GIL=0 uv run bench_sync_async.py
```

Output:

```
================================================================================
Scraper-rs Benchmark: Sync vs Async Performance
================================================================================

SMALL HTML (~200 bytes)
--------------------------------------------------------------------------------
Synchronous functions:
  select                        :      7.01 ms total,     70.14 µs avg
  select_first                  :      6.78 ms total,     67.76 µs avg
  first                         :      6.36 ms total,     63.59 µs avg
  xpath                         :      9.75 ms total,     97.53 µs avg
  xpath_first                   :      9.65 ms total,     96.49 µs avg

Asynchronous functions (sequential):
  async select                  :    108.81 ms total,      1.09 ms avg
  async select_first            :    106.49 ms total,      1.06 ms avg
  async first                   :    107.47 ms total,      1.07 ms avg
  async xpath                   :     99.69 ms total,    996.92 µs avg
  async xpath_first             :     89.52 ms total,    895.23 µs avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :      4.69 ms total,    468.85 µs avg
  concurrent xpath              :      5.13 ms total,    513.30 µs avg

MEDIUM HTML (~5KB, 100 items)
--------------------------------------------------------------------------------
Synchronous functions:
  select                        :    194.99 ms total,      1.95 ms avg
  xpath                         :    236.09 ms total,      2.36 ms avg

Asynchronous functions (sequential):
  async select                  :    297.73 ms total,      2.98 ms avg
  async xpath                   :    356.43 ms total,      3.56 ms avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :     22.04 ms total,      2.20 ms avg

LARGE HTML (~50KB, 1000 items)
--------------------------------------------------------------------------------
Synchronous functions:
  select                        :       1.33 s total,     26.60 ms avg
  xpath                         :       2.88 s total,     57.63 ms avg

Asynchronous functions (sequential):
  async select                  :       2.16 s total,     43.25 ms avg
  async xpath                   :       1.78 s total,     35.60 ms avg

Asynchronous functions (concurrent, 10 tasks):
  concurrent select             :    250.87 ms total,     25.09 ms avg

================================================================================
Summary
================================================================================

Note: Async functions show their value in concurrent scenarios where
      multiple operations can be performed simultaneously without blocking.
      For CPU-bound operations like HTML parsing, sync functions may be
      faster for sequential execution, but async allows better responsiveness
      in I/O-bound applications.
```
