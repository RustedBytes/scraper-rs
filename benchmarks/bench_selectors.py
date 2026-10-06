"""Selector generation latency; run against a release extension.

Parsing/target lookup are excluded. Cold includes lazy XPath initialization;
warm is repeated generation on the same document (including compilation cache).
No wall-clock pass/fail threshold: shared CI hardware is noisy.
"""

import argparse
import json
import statistics
import time

from scraper_rs import Document


def fixture(depth: int, attributes: int) -> str:
    # Mirrored branches force ambiguous candidates and positional fallback.
    def branch(label: str) -> str:
        wrappers = [
            "<div "
            + " ".join(f'data-a{a}="level-{d}-{a}"' for a in range(attributes))
            + ">"
            for d in range(depth)
        ]
        leaf = (
            "<span "
            + " ".join(f'data-a{a}="leaf-{a}"' for a in range(attributes))
            + f">{label}</span>"
        )
        return "".join(wrappers) + leaf + "</div>" * depth

    return "<main>" + branch("decoy") + branch("target") + "</main>"


def run(repeats: int) -> list[dict]:
    results = []
    for scenario, depth, attributes in [
        ("deep-16", 16, 0),
        ("deep-64", 64, 0),
        ("deep-128", 128, 0),
        ("attrs-8", 4, 8),
        ("attrs-32", 4, 32),
        ("attrs-128", 4, 128),
        ("deep-attrs", 64, 16),
    ]:
        for kind in ("css", "xpath"):
            doc = Document(fixture(depth, attributes))
            target = doc.select("span")[1]
            generate = getattr(doc, f"generate_{kind}_selector")
            start = time.perf_counter_ns()
            selector = generate(target)
            cold = (time.perf_counter_ns() - start) / 1e6
            if selector is not None:
                matches = doc.select(selector) if kind == "css" else doc.xpath(selector)
                assert len(matches) == 1 and matches[0].text == "target", (
                    scenario,
                    selector,
                )
            else:
                assert kind == "css", scenario
            samples = []
            for _ in range(repeats):
                start = time.perf_counter_ns()
                generate(target)
                samples.append((time.perf_counter_ns() - start) / 1e6)
            row = {
                "scenario": scenario,
                "kind": kind,
                "depth": depth,
                "attributes": attributes,
                "cold_ms": cold,
                "median_ms": statistics.median(samples),
                "max_ms": max(samples),
            }
            results.append(row)
            print(json.dumps(row), flush=True)
    return results


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--output")
    args = parser.parse_args()
    rows = run(args.repeats)
    if args.output:
        with open(args.output, "w") as out:
            json.dump(rows, out, indent=2)
