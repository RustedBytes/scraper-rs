#!/usr/bin/env python3
"""
Benchmark script comparing scraper-rs with markupever.

This script compares the performance of scraper-rs against markupever,
another Python HTML parsing library based on html5ever.
"""

import time
from typing import Callable, Any

# Import libraries
try:
    import scraper_rs
    SCRAPER_RS_AVAILABLE = True
except ImportError:
    SCRAPER_RS_AVAILABLE = False
    print("Warning: scraper-rs not available")

try:
    import markupever
    MARKUPEVER_AVAILABLE = True
except ImportError:
    MARKUPEVER_AVAILABLE = False
    print("Warning: markupever not available")

# Sample HTML documents of varying sizes
SMALL_HTML = """
<html>
  <body>
    <div class="item" data-id="1"><a href="/a">First</a></div>
    <div class="item" data-id="2"><a href="/b">Second</a></div>
  </body>
</html>
"""

MEDIUM_HTML = """
<html>
  <head><title>Test Page</title></head>
  <body>
    <nav>
      <ul>
        <li><a href="/home">Home</a></li>
        <li><a href="/about">About</a></li>
      </ul>
    </nav>
    <main>
      {}
    </main>
  </body>
</html>
""".format(
    "\n".join(
        f'<div class="item" data-id="{i}"><a href="/item{i}">Item {i}</a></div>'
        for i in range(100)
    )
)

LARGE_HTML = """
<html>
  <head><title>Large Test Page</title></head>
  <body>
    <nav>
      <ul>
        <li><a href="/home">Home</a></li>
        <li><a href="/about">About</a></li>
      </ul>
    </nav>
    <main>
      {}
    </main>
  </body>
</html>
""".format(
    "\n".join(
        f'<div class="item" data-id="{i}"><a href="/item{i}">Item {i}</a><p>Description for item {i}</p></div>'
        for i in range(1000)
    )
)


def benchmark_function(
    func: Callable, *args: Any, iterations: int = 100, **kwargs: Any
) -> float:
    """Benchmark a function by running it multiple times."""
    start = time.perf_counter()
    for _ in range(iterations):
        func(*args, **kwargs)
    end = time.perf_counter()
    return end - start


def format_time(seconds: float) -> str:
    """Format time in human-readable format."""
    if seconds < 0.001:
        return f"{seconds * 1_000_000:.2f} µs"
    elif seconds < 1:
        return f"{seconds * 1_000:.2f} ms"
    else:
        return f"{seconds:.2f} s"


def print_result(name: str, time_taken: float, iterations: int = 100) -> None:
    """Print benchmark result."""
    avg_time = time_taken / iterations
    print(
        f"  {name:30s}: {format_time(time_taken):>12s} total, {format_time(avg_time):>12s} avg"
    )


def benchmark_scraper_rs(html: str, iterations: int) -> dict:
    """Benchmark scraper-rs operations."""
    results = {}
    
    # Parse document
    parse_time = benchmark_function(scraper_rs.Document, html, iterations=iterations)
    results['parse'] = parse_time
    
    # CSS selection
    css_time = benchmark_function(scraper_rs.select, html, ".item", iterations=iterations)
    results['css_select'] = css_time
    
    # CSS select_first
    css_first_time = benchmark_function(scraper_rs.select_first, html, ".item", iterations=iterations)
    results['css_select_first'] = css_first_time
    
    return results


def benchmark_markupever(html: str, iterations: int) -> dict:
    """Benchmark markupever operations."""
    results = {}
    
    # Parse document
    def parse_markupever(html_str):
        return markupever.parse(html_str)
    
    parse_time = benchmark_function(parse_markupever, html, iterations=iterations)
    results['parse'] = parse_time
    
    # CSS selection
    def select_markupever(html_str):
        doc = markupever.parse(html_str)
        return doc.select(".item")
    
    css_time = benchmark_function(select_markupever, html, iterations=iterations)
    results['css_select'] = css_time
    
    # CSS select_first
    def select_first_markupever(html_str):
        doc = markupever.parse(html_str)
        return doc.select_one(".item")
    
    css_first_time = benchmark_function(select_first_markupever, html, iterations=iterations)
    results['css_select_first'] = css_first_time
    
    return results


def print_comparison(scraper_rs_results: dict, markupever_results: dict, iterations: int):
    """Print comparison between scraper-rs and markupever."""
    print("\n  Operation              scraper-rs           markupever         Ratio (scraper-rs/markupever)")
    print("  " + "-" * 85)
    
    for operation in scraper_rs_results.keys():
        sr_time = scraper_rs_results[operation]
        me_time = markupever_results[operation]
        ratio = sr_time / me_time if me_time > 0 else 0
        
        sr_avg = sr_time / iterations
        me_avg = me_time / iterations
        
        print(f"  {operation:20s}  {format_time(sr_avg):>12s}     {format_time(me_avg):>12s}       {ratio:.2f}x")


def main() -> None:
    """Run all benchmarks."""
    if not SCRAPER_RS_AVAILABLE:
        print("Error: scraper-rs is not installed")
        print("Please build the package first with: maturin develop --release")
        return
    
    if not MARKUPEVER_AVAILABLE:
        print("Error: markupever is not installed")
        print("Please install it with: pip install markupever")
        return
    
    print("=" * 90)
    print("scraper-rs vs markupever Benchmark")
    print("=" * 90)
    print()
    
    iterations = 100
    iterations_large = 50
    
    # Benchmark with small HTML
    print("SMALL HTML (~200 bytes)")
    print("-" * 90)
    print("scraper-rs:")
    sr_small = benchmark_scraper_rs(SMALL_HTML, iterations)
    for op, time_val in sr_small.items():
        print_result(op, time_val, iterations)
    
    print("\nmarkupever:")
    me_small = benchmark_markupever(SMALL_HTML, iterations)
    for op, time_val in me_small.items():
        print_result(op, time_val, iterations)
    
    print_comparison(sr_small, me_small, iterations)
    print()
    
    # Benchmark with medium HTML
    print("MEDIUM HTML (~5KB, 100 items)")
    print("-" * 90)
    print("scraper-rs:")
    sr_medium = benchmark_scraper_rs(MEDIUM_HTML, iterations)
    for op, time_val in sr_medium.items():
        print_result(op, time_val, iterations)
    
    print("\nmarkupever:")
    me_medium = benchmark_markupever(MEDIUM_HTML, iterations)
    for op, time_val in me_medium.items():
        print_result(op, time_val, iterations)
    
    print_comparison(sr_medium, me_medium, iterations)
    print()
    
    # Benchmark with large HTML
    print("LARGE HTML (~50KB, 1000 items)")
    print("-" * 90)
    print("scraper-rs:")
    sr_large = benchmark_scraper_rs(LARGE_HTML, iterations_large)
    for op, time_val in sr_large.items():
        print_result(op, time_val, iterations_large)
    
    print("\nmarkupever:")
    me_large = benchmark_markupever(LARGE_HTML, iterations_large)
    for op, time_val in me_large.items():
        print_result(op, time_val, iterations_large)
    
    print_comparison(sr_large, me_large, iterations_large)
    print()
    
    print("=" * 90)
    print("Summary")
    print("=" * 90)
    print()
    print("This benchmark compares scraper-rs (after optimizations) with markupever.")
    print("Both libraries are based on html5ever for HTML parsing.")
    print()
    print("Key observations:")
    print("- scraper-rs now has lazy XPath parsing (only parsed when needed)")
    print("- scraper-rs uses lazy property computation for Element attributes")
    print("- Ratios < 2.0x indicate scraper-rs is competitive with markupever")
    print("- The 'atomic' feature is enabled for enhanced thread safety")
    print()


if __name__ == "__main__":
    main()
