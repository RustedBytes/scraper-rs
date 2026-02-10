"""
Benchmark parser memory usage with progressively larger HTML inputs.

Each HTML size is benchmarked in an isolated subprocess so RSS measurements do
not leak across sizes. The child process repeatedly parses the same document
and reports parse latency plus peak RSS growth.
"""

from __future__ import annotations

import argparse
import gc
import json
import math
import os
import statistics
import subprocess
import sys
import threading
import time
from typing import Any

KIB = 1024
MIB = 1024 * KIB

DEFAULT_MIN_KIB = 2
DEFAULT_MAX_KIB = 8192
DEFAULT_GROWTH_FACTOR = 2.0
DEFAULT_TARGET_TOTAL_MIB = 64
DEFAULT_MIN_ITERATIONS = 5
DEFAULT_MAX_ITERATIONS = 120
DEFAULT_WARMUP = 3
DEFAULT_SAMPLE_MS = 1.0


def format_bytes(num_bytes: int) -> str:
    """Return a compact human-readable size string."""
    if num_bytes >= MIB:
        return f"{num_bytes / MIB:.2f} MiB"
    if num_bytes >= KIB:
        return f"{num_bytes / KIB:.2f} KiB"
    return f"{num_bytes} B"


def parse_size_token(token: str) -> int:
    """Parse a token like 256k, 2m, or 4096 (bytes)."""
    raw = token.strip().lower()
    if not raw:
        raise ValueError("empty size token")

    multiplier = 1
    if raw.endswith("k"):
        multiplier = KIB
        raw = raw[:-1]
    elif raw.endswith("m"):
        multiplier = MIB
        raw = raw[:-1]
    elif raw.endswith("g"):
        multiplier = MIB * KIB
        raw = raw[:-1]

    value = float(raw)
    if value <= 0:
        raise ValueError(f"size must be positive: {token!r}")
    return int(value * multiplier)


def parse_sizes_argument(raw: str) -> list[int]:
    """Parse comma-separated sizes into a sorted unique list of bytes."""
    sizes = [parse_size_token(token) for token in raw.split(",") if token.strip()]
    if not sizes:
        raise ValueError("no sizes were provided")
    return sorted(set(sizes))


def build_progressive_sizes(
    min_bytes: int, max_bytes: int, growth_factor: float
) -> list[int]:
    """Build monotonically increasing progressive sizes."""
    if min_bytes <= 0 or max_bytes <= 0 or min_bytes > max_bytes:
        raise ValueError("invalid size bounds")
    if growth_factor <= 1.0:
        raise ValueError("growth_factor must be > 1.0")

    sizes: list[int] = []
    current = float(min_bytes)

    while current < max_bytes:
        candidate = int(round(current))
        if sizes and candidate <= sizes[-1]:
            candidate = sizes[-1] + 1
        sizes.append(candidate)
        current *= growth_factor

    if not sizes or sizes[-1] != max_bytes:
        sizes.append(max_bytes)
    return sizes


def choose_iterations(
    size_bytes: int,
    target_total_bytes: int,
    min_iterations: int,
    max_iterations: int,
) -> int:
    """Scale iterations by input size while keeping runtime bounded."""
    estimated = max(1, int(target_total_bytes / max(1, size_bytes)))
    return max(min_iterations, min(max_iterations, estimated))


def build_html(target_bytes: int) -> str:
    """Generate deterministic HTML close to the target size."""
    head = (
        "<!doctype html><html><head><meta charset='utf-8'>"
        "<title>scraper-rs memory benchmark</title></head><body><main>"
    )
    tail = "</main></body></html>"
    article_template = (
        "<article class='item' data-id='{id}'>"
        "<h2>Item {id}</h2>"
        "<p>Lorem ipsum dolor sit amet, consectetur adipiscing elit.</p>"
        "<ul><li>alpha</li><li>beta</li><li>gamma</li></ul>"
        "</article>"
    )

    static_bytes = len(head) + len(tail)
    if target_bytes <= static_bytes:
        return head + tail

    parts = [head]
    total_bytes = static_bytes
    idx = 0

    while True:
        article = article_template.format(id=idx)
        article_bytes = len(article)
        if total_bytes + article_bytes > target_bytes:
            break
        parts.append(article)
        total_bytes += article_bytes
        idx += 1

    remaining = target_bytes - total_bytes
    if remaining > 0:
        if remaining >= 7:
            parts.append(f"<!--{'x' * (remaining - 7)}-->")
        else:
            parts.append("x" * remaining)

    parts.append(tail)
    return "".join(parts)


def get_current_rss_bytes() -> int:
    """Return current RSS bytes if possible."""
    statm_path = "/proc/self/statm"
    if os.path.exists(statm_path):
        try:
            with open(statm_path, "r", encoding="ascii") as statm_file:
                rss_pages = int(statm_file.readline().split()[1])
            return rss_pages * os.sysconf("SC_PAGE_SIZE")
        except (OSError, ValueError, IndexError):
            pass

    # Fallback: ru_maxrss reports peak RSS, not current RSS.
    try:
        import resource
    except ImportError:
        return 0

    max_rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    if sys.platform == "darwin":
        return int(max_rss)
    return int(max_rss * 1024)


def percentile(values: list[float], pct: float) -> float:
    """Compute percentile with linear interpolation."""
    if not values:
        return 0.0
    ordered = sorted(values)
    rank = (len(ordered) - 1) * (pct / 100.0)
    lower = math.floor(rank)
    upper = math.ceil(rank)
    if lower == upper:
        return ordered[lower]
    weight = rank - lower
    return ordered[lower] + (ordered[upper] - ordered[lower]) * weight


class RssSampler:
    """Continuously sample RSS and keep a peak."""

    def __init__(self, interval_seconds: float) -> None:
        self.interval_seconds = max(interval_seconds, 0.0005)
        self.peak_rss_bytes = 0
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None

    def start(self) -> None:
        self.peak_rss_bytes = get_current_rss_bytes()
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        if self._thread is not None:
            self._thread.join()

    def _run(self) -> None:
        while not self._stop.is_set():
            rss_now = get_current_rss_bytes()
            if rss_now > self.peak_rss_bytes:
                self.peak_rss_bytes = rss_now
            time.sleep(self.interval_seconds)

        rss_now = get_current_rss_bytes()
        if rss_now > self.peak_rss_bytes:
            self.peak_rss_bytes = rss_now


def run_child(size_bytes: int, iterations: int, warmup: int, sample_ms: float) -> int:
    """Run benchmark for a single size and print JSON result."""
    try:
        import scraper_rs
    except ImportError as exc:
        print(json.dumps({"error": f"unable to import scraper_rs: {exc}"}))
        return 1

    html = build_html(size_bytes)
    html_bytes = len(html.encode("utf-8"))
    iterations = max(1, iterations)
    warmup = max(0, warmup)

    for _ in range(warmup):
        doc = scraper_rs.Document(html)
        doc.close()
    gc.collect()

    baseline_rss = get_current_rss_bytes()
    sampler = RssSampler(sample_ms / 1000.0)
    sampler.start()

    parse_ms: list[float] = []
    for _ in range(iterations):
        started = time.perf_counter()
        doc = scraper_rs.Document(html)
        doc.close()
        parse_ms.append((time.perf_counter() - started) * 1000.0)

    sampler.stop()
    gc.collect()

    peak_rss = max(sampler.peak_rss_bytes, get_current_rss_bytes())
    result = {
        "target_bytes": size_bytes,
        "html_bytes": html_bytes,
        "iterations": iterations,
        "median_parse_ms": statistics.median(parse_ms),
        "p95_parse_ms": percentile(parse_ms, 95.0),
        "min_parse_ms": min(parse_ms),
        "max_parse_ms": max(parse_ms),
        "baseline_rss_bytes": baseline_rss,
        "peak_rss_bytes": peak_rss,
        "rss_delta_bytes": max(0, peak_rss - baseline_rss),
    }
    print(json.dumps(result))
    return 0


def run_child_process(
    python_executable: str,
    script_path: str,
    size_bytes: int,
    iterations: int,
    warmup: int,
    sample_ms: float,
) -> dict[str, Any]:
    """Run one child benchmark process and parse the JSON result."""
    command = [
        python_executable,
        script_path,
        "--child",
        "--size-bytes",
        str(size_bytes),
        "--iterations",
        str(iterations),
        "--warmup",
        str(warmup),
        "--sample-ms",
        str(sample_ms),
    ]
    completed = subprocess.run(command, capture_output=True, text=True, check=False)

    if completed.returncode != 0:
        raise RuntimeError(
            f"child failed for {format_bytes(size_bytes)} (exit {completed.returncode})\n"
            f"stdout:\n{completed.stdout}\n"
            f"stderr:\n{completed.stderr}"
        )

    lines = [line.strip() for line in completed.stdout.splitlines() if line.strip()]
    if not lines:
        raise RuntimeError(f"child returned no JSON for {format_bytes(size_bytes)}")

    try:
        result = json.loads(lines[-1])
    except json.JSONDecodeError as exc:
        raise RuntimeError(
            f"invalid JSON from child for {format_bytes(size_bytes)}: {lines[-1]}"
        ) from exc

    if "error" in result:
        raise RuntimeError(str(result["error"]))

    return result


def print_results(results: list[dict[str, Any]]) -> None:
    """Print benchmark table."""
    print()
    print(
        f"{'Input Size':>12} {'Runs':>6} {'Median ms':>10} {'P95 ms':>8} "
        f"{'Base RSS':>10} {'Peak RSS':>10} {'Delta RSS':>10} {'Delta/Input':>11}"
    )
    print("-" * 94)

    for row in results:
        html_bytes = int(row["html_bytes"])
        delta_bytes = int(row["rss_delta_bytes"])
        delta_per_input = delta_bytes / max(1, html_bytes)
        print(
            f"{format_bytes(html_bytes):>12} {int(row['iterations']):6d} "
            f"{float(row['median_parse_ms']):10.3f} {float(row['p95_parse_ms']):8.3f} "
            f"{int(row['baseline_rss_bytes']) / MIB:10.2f} "
            f"{int(row['peak_rss_bytes']) / MIB:10.2f} "
            f"{delta_bytes / MIB:10.2f} {delta_per_input:11.2f}x"
        )


def run_parent(args: argparse.Namespace) -> int:
    """Run full progressive benchmark across all sizes."""
    if args.sizes:
        sizes = parse_sizes_argument(args.sizes)
    else:
        sizes = build_progressive_sizes(
            min_bytes=args.min_kib * KIB,
            max_bytes=args.max_kib * KIB,
            growth_factor=args.growth_factor,
        )

    target_total_bytes = int(args.target_total_mib * MIB)
    script_path = os.path.abspath(__file__)
    results: list[dict[str, Any]] = []

    print("=" * 94)
    print("scraper-rs Parse Memory Benchmark (progressive HTML sizes)")
    print("=" * 94)
    if args.sizes:
        print(f"Sizes: {', '.join(format_bytes(size) for size in sizes)}")
    else:
        print(
            "Sizes: "
            f"{format_bytes(sizes[0])} -> {format_bytes(sizes[-1])} "
            f"(growth factor {args.growth_factor:g})"
        )
    print(
        f"Iterations per size: scaled to ~{args.target_total_mib} MiB parsed "
        f"(min={args.min_iterations}, max={args.max_iterations})"
    )
    print()

    for size in sizes:
        iterations = choose_iterations(
            size_bytes=size,
            target_total_bytes=target_total_bytes,
            min_iterations=args.min_iterations,
            max_iterations=args.max_iterations,
        )
        print(
            f"[run] target={format_bytes(size):>10} iterations={iterations:3d} "
            f"warmup={args.warmup}"
        )
        result = run_child_process(
            python_executable=args.python,
            script_path=script_path,
            size_bytes=size,
            iterations=iterations,
            warmup=args.warmup,
            sample_ms=args.sample_ms,
        )
        results.append(result)

    print_results(results)
    print()
    print("Notes:")
    print("- Each size runs in a fresh subprocess to keep RSS measurements isolated.")
    print("- Delta RSS is peak RSS minus baseline RSS measured around the parse loop.")
    if not os.path.exists("/proc/self/statm"):
        print(
            "- /proc/self/statm was not available; RSS fallback may be less precise "
            "on this platform."
        )
    return 0


def parse_args() -> argparse.Namespace:
    """Build CLI parser and parse arguments."""
    parser = argparse.ArgumentParser(
        description=(
            "Benchmark parser memory usage across progressively larger HTML sizes."
        )
    )
    parser.add_argument(
        "--min-kib",
        type=int,
        default=DEFAULT_MIN_KIB,
        help=f"Smallest HTML size in KiB (default: {DEFAULT_MIN_KIB})",
    )
    parser.add_argument(
        "--max-kib",
        type=int,
        default=DEFAULT_MAX_KIB,
        help=f"Largest HTML size in KiB (default: {DEFAULT_MAX_KIB})",
    )
    parser.add_argument(
        "--growth-factor",
        type=float,
        default=DEFAULT_GROWTH_FACTOR,
        help=f"Size multiplier for progressive steps (default: {DEFAULT_GROWTH_FACTOR})",
    )
    parser.add_argument(
        "--sizes",
        type=str,
        default="",
        help="Optional explicit comma-separated sizes (e.g. 4k,16k,256k,1m)",
    )
    parser.add_argument(
        "--target-total-mib",
        type=int,
        default=DEFAULT_TARGET_TOTAL_MIB,
        help=(
            "Approximate total input volume parsed per size in MiB to scale runs "
            f"(default: {DEFAULT_TARGET_TOTAL_MIB})"
        ),
    )
    parser.add_argument(
        "--min-iterations",
        type=int,
        default=DEFAULT_MIN_ITERATIONS,
        help=f"Minimum parses per size (default: {DEFAULT_MIN_ITERATIONS})",
    )
    parser.add_argument(
        "--max-iterations",
        type=int,
        default=DEFAULT_MAX_ITERATIONS,
        help=f"Maximum parses per size (default: {DEFAULT_MAX_ITERATIONS})",
    )
    parser.add_argument(
        "--warmup",
        type=int,
        default=DEFAULT_WARMUP,
        help=f"Warmup parses in each child process (default: {DEFAULT_WARMUP})",
    )
    parser.add_argument(
        "--sample-ms",
        type=float,
        default=DEFAULT_SAMPLE_MS,
        help=f"RSS sampling interval in milliseconds (default: {DEFAULT_SAMPLE_MS})",
    )
    parser.add_argument(
        "--python",
        type=str,
        default=sys.executable,
        help="Python executable used for child runs (default: current interpreter)",
    )

    # Internal child-run arguments.
    parser.add_argument("--child", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--size-bytes", type=int, help=argparse.SUPPRESS)
    parser.add_argument("--iterations", type=int, help=argparse.SUPPRESS)
    return parser.parse_args()


def main() -> int:
    """CLI entrypoint."""
    args = parse_args()

    if args.child:
        if args.size_bytes is None or args.iterations is None:
            print(
                json.dumps({"error": "--child requires --size-bytes and --iterations"})
            )
            return 2
        return run_child(
            size_bytes=args.size_bytes,
            iterations=args.iterations,
            warmup=args.warmup,
            sample_ms=args.sample_ms,
        )

    if args.min_kib > args.max_kib:
        print("Error: --min-kib must be <= --max-kib", file=sys.stderr)
        return 2
    if args.min_iterations > args.max_iterations:
        print("Error: --min-iterations must be <= --max-iterations", file=sys.stderr)
        return 2
    if args.target_total_mib <= 0:
        print("Error: --target-total-mib must be > 0", file=sys.stderr)
        return 2
    if args.growth_factor <= 1.0:
        print("Error: --growth-factor must be > 1.0", file=sys.stderr)
        return 2

    try:
        return run_parent(args)
    except (ValueError, RuntimeError) as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
