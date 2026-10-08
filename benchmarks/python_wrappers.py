"""Measure real asyncio wrappers and isolated Python wrapping (not Criterion)."""

import argparse
import asyncio
import gc
import hashlib
import json
import platform
import statistics
import time
from pathlib import Path


def summarize(samples):
    return {
        "samples_ns_per_op": samples,
        "median_ns_per_op": statistics.median(samples),
    }


async def run(iterations, repeats):
    import scraper_rs
    from scraper_rs import asyncio as api

    html = '<div class="item">Україна</div>' * 100
    doc = await api.parse(html)
    core = await doc._document.select(".item")
    assert len(core) == len(await doc.select(".item")) == 100

    async def core_select():
        return await doc._document.select(".item")

    async def public_select():
        return await doc.select(".item")

    async def public_parse():
        result = await api.parse(html)
        result.close()

    async def core_parse():
        result = await api._parse_async(html)
        result.close()

    async def ready():
        return None

    cases = {
        "asyncio_ready_control": ready,
        "core_select_existing": core_select,
        "public_select_existing": public_select,
        "core_parse_and_close": core_parse,
        "public_parse_and_close": public_parse,
    }
    samples = {name: [] for name in cases}
    samples["python_wrap_elements_only"] = []
    for operation in cases.values():
        for _ in range(20):
            await operation()
    # Alternate order to expose drift. Each output is dropped in the loop.
    for repeat in range(repeats):
        order = list(cases.items())
        if repeat % 2:
            order.reverse()
        for name, operation in order:
            start = time.perf_counter_ns()
            for _ in range(iterations):
                await operation()
            samples[name].append((time.perf_counter_ns() - start) / iterations)
        start = time.perf_counter_ns()
        for _ in range(iterations):
            api._wrap_elements(core)
        samples["python_wrap_elements_only"].append(
            (time.perf_counter_ns() - start) / iterations
        )
    doc.close()
    return {
        "schema": 1,
        "python": platform.python_version(),
        "module": scraper_rs.__file__,
        "extension_sha256": hashlib.sha256(
            Path(api._core.__file__).read_bytes()
        ).hexdigest(),
        "version": scraper_rs.__version__,
        "html_bytes": len(html.encode()),
        "matches": 100,
        "iterations": iterations,
        "repeats": repeats,
        "gc_enabled": gc.isenabled(),
        "metrics": {name: summarize(values) for name, values in samples.items()},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--iterations", type=int, default=1000)
    parser.add_argument("--repeats", type=int, default=6)
    args = parser.parse_args()
    if args.iterations < 1 or args.repeats < 2:
        parser.error("iterations >= 1 and repeats >= 2 required")
    print(json.dumps(asyncio.run(run(args.iterations, args.repeats)), indent=2))


if __name__ == "__main__":
    main()
