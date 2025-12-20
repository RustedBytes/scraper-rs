#!/usr/bin/env python3
"""
Benchmark script to compare sync vs async performance of scraper_rs functions.

This script measures the performance of:
- Sync functions: select, select_first, first, xpath, xpath_first
- Async functions: async select, async select_first, async first, async xpath, async xpath_first
"""

import asyncio
import time
from typing import Any, Callable

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


def benchmark_sync(func: Callable, *args: Any, iterations: int = 100, **kwargs: Any) -> float:
    """Benchmark a synchronous function."""
    start = time.perf_counter()
    for _ in range(iterations):
        func(*args, **kwargs)
    end = time.perf_counter()
    return end - start


async def benchmark_async(
    func: Callable, *args: Any, iterations: int = 100, **kwargs: Any
) -> float:
    """Benchmark an asynchronous function."""
    start = time.perf_counter()
    for _ in range(iterations):
        await func(*args, **kwargs)
    end = time.perf_counter()
    return end - start


async def benchmark_async_concurrent(
    func: Callable, *args: Any, concurrency: int = 10, **kwargs: Any
) -> float:
    """Benchmark an asynchronous function with concurrent execution."""
    start = time.perf_counter()
    tasks = [func(*args, **kwargs) for _ in range(concurrency)]
    await asyncio.gather(*tasks)
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
    print(f"  {name:30s}: {format_time(time_taken):>12s} total, {format_time(avg_time):>12s} avg")


async def main() -> None:
    """Run all benchmarks."""
    # Import modules
    try:
        import scraper_rs
        from scraper_rs import asyncio as async_scraper
    except ImportError as e:
        print(f"Error importing scraper_rs: {e}")
        print("Please build the package first with: maturin develop --release")
        return

    iterations = 100
    concurrent_tasks = 10

    print("=" * 80)
    print("Scraper-rs Benchmark: Sync vs Async Performance")
    print("=" * 80)
    print()

    # Benchmark with small HTML
    print("SMALL HTML (~200 bytes)")
    print("-" * 80)

    # Sync benchmarks
    print("Synchronous functions:")
    time_select = benchmark_sync(scraper_rs.select, SMALL_HTML, ".item", iterations=iterations)
    print_result("select", time_select, iterations)

    time_select_first = benchmark_sync(
        scraper_rs.select_first, SMALL_HTML, ".item", iterations=iterations
    )
    print_result("select_first", time_select_first, iterations)

    time_first = benchmark_sync(scraper_rs.first, SMALL_HTML, ".item", iterations=iterations)
    print_result("first", time_first, iterations)

    time_xpath = benchmark_sync(scraper_rs.xpath, SMALL_HTML, "//div[@class='item']", iterations=iterations)
    print_result("xpath", time_xpath, iterations)

    time_xpath_first = benchmark_sync(
        scraper_rs.xpath_first, SMALL_HTML, "//div[@class='item']", iterations=iterations
    )
    print_result("xpath_first", time_xpath_first, iterations)

    # Async benchmarks
    print("\nAsynchronous functions (sequential):")
    time_async_select = await benchmark_async(
        async_scraper.select, SMALL_HTML, ".item", iterations=iterations
    )
    print_result("async select", time_async_select, iterations)

    time_async_select_first = await benchmark_async(
        async_scraper.select_first, SMALL_HTML, ".item", iterations=iterations
    )
    print_result("async select_first", time_async_select_first, iterations)

    time_async_first = await benchmark_async(
        async_scraper.first, SMALL_HTML, ".item", iterations=iterations
    )
    print_result("async first", time_async_first, iterations)

    time_async_xpath = await benchmark_async(
        async_scraper.xpath, SMALL_HTML, "//div[@class='item']", iterations=iterations
    )
    print_result("async xpath", time_async_xpath, iterations)

    time_async_xpath_first = await benchmark_async(
        async_scraper.xpath_first, SMALL_HTML, "//div[@class='item']", iterations=iterations
    )
    print_result("async xpath_first", time_async_xpath_first, iterations)

    # Async concurrent benchmarks
    print(f"\nAsynchronous functions (concurrent, {concurrent_tasks} tasks):")
    time_concurrent_select = await benchmark_async_concurrent(
        async_scraper.select, SMALL_HTML, ".item", concurrency=concurrent_tasks
    )
    print_result("concurrent select", time_concurrent_select, concurrent_tasks)

    time_concurrent_xpath = await benchmark_async_concurrent(
        async_scraper.xpath, SMALL_HTML, "//div[@class='item']", concurrency=concurrent_tasks
    )
    print_result("concurrent xpath", time_concurrent_xpath, concurrent_tasks)

    print()

    # Benchmark with medium HTML
    print("MEDIUM HTML (~5KB, 100 items)")
    print("-" * 80)

    print("Synchronous functions:")
    time_select_med = benchmark_sync(scraper_rs.select, MEDIUM_HTML, ".item", iterations=iterations)
    print_result("select", time_select_med, iterations)

    time_xpath_med = benchmark_sync(scraper_rs.xpath, MEDIUM_HTML, "//div[@class='item']", iterations=iterations)
    print_result("xpath", time_xpath_med, iterations)

    print("\nAsynchronous functions (sequential):")
    time_async_select_med = await benchmark_async(
        async_scraper.select, MEDIUM_HTML, ".item", iterations=iterations
    )
    print_result("async select", time_async_select_med, iterations)

    time_async_xpath_med = await benchmark_async(
        async_scraper.xpath, MEDIUM_HTML, "//div[@class='item']", iterations=iterations
    )
    print_result("async xpath", time_async_xpath_med, iterations)

    print(f"\nAsynchronous functions (concurrent, {concurrent_tasks} tasks):")
    time_concurrent_select_med = await benchmark_async_concurrent(
        async_scraper.select, MEDIUM_HTML, ".item", concurrency=concurrent_tasks
    )
    print_result("concurrent select", time_concurrent_select_med, concurrent_tasks)

    print()

    # Benchmark with large HTML
    print("LARGE HTML (~50KB, 1000 items)")
    print("-" * 80)

    iterations_large = 50  # Fewer iterations for large HTML

    print("Synchronous functions:")
    time_select_large = benchmark_sync(scraper_rs.select, LARGE_HTML, ".item", iterations=iterations_large)
    print_result("select", time_select_large, iterations_large)

    time_xpath_large = benchmark_sync(
        scraper_rs.xpath, LARGE_HTML, "//div[@class='item']", iterations=iterations_large
    )
    print_result("xpath", time_xpath_large, iterations_large)

    print("\nAsynchronous functions (sequential):")
    time_async_select_large = await benchmark_async(
        async_scraper.select, LARGE_HTML, ".item", iterations=iterations_large
    )
    print_result("async select", time_async_select_large, iterations_large)

    time_async_xpath_large = await benchmark_async(
        async_scraper.xpath, LARGE_HTML, "//div[@class='item']", iterations=iterations_large
    )
    print_result("async xpath", time_async_xpath_large, iterations_large)

    print(f"\nAsynchronous functions (concurrent, {concurrent_tasks} tasks):")
    time_concurrent_select_large = await benchmark_async_concurrent(
        async_scraper.select, LARGE_HTML, ".item", concurrency=concurrent_tasks
    )
    print_result("concurrent select", time_concurrent_select_large, concurrent_tasks)

    print()
    print("=" * 80)
    print("Summary")
    print("=" * 80)
    print()
    print("Note: Async functions show their value in concurrent scenarios where")
    print("      multiple operations can be performed simultaneously without blocking.")
    print("      For CPU-bound operations like HTML parsing, sync functions may be")
    print("      faster for sequential execution, but async allows better responsiveness")
    print("      in I/O-bound applications.")
    print()


if __name__ == "__main__":
    asyncio.run(main())
