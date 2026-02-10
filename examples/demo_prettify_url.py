"""Fetch a website and print a prettified DOM preview."""

from __future__ import annotations

import argparse
from urllib.request import Request, urlopen

from scraper_rs import prettify


def fetch_html(url: str, timeout: float, max_bytes: int) -> tuple[str, bool]:
    request = Request(url, headers={"User-Agent": "scraper-rs-demo/1.0"})
    with urlopen(request, timeout=timeout) as response:  # nosec: B310 (example script)
        raw = response.read(max_bytes + 1)
        truncated = len(raw) > max_bytes
        if truncated:
            raw = raw[:max_bytes]
        charset = response.headers.get_content_charset() or "utf-8"
    return raw.decode(charset, errors="replace"), truncated


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Load a URL and print a prettified DOM preview."
    )
    parser.add_argument(
        "url",
        nargs="?",
        default="https://example.com",
        help="Target URL (default: https://example.com)",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=10.0,
        help="HTTP timeout in seconds (default: 10)",
    )
    parser.add_argument(
        "--max-bytes",
        type=int,
        default=1_000_000,
        help="Maximum bytes to download and prettify (default: 1000000)",
    )
    parser.add_argument(
        "--max-lines",
        type=int,
        default=120,
        help="Maximum output lines to print (default: 120)",
    )
    args = parser.parse_args()

    html, truncated = fetch_html(args.url, args.timeout, args.max_bytes)
    pretty = prettify(
        html,
        max_size_bytes=args.max_bytes,
        truncate_on_limit=True,
    )

    lines = pretty.splitlines()
    print(f"URL: {args.url}")
    print(f"Downloaded bytes: {len(html.encode('utf-8'))}")
    if truncated:
        print("Input was truncated to --max-bytes before prettify.")
    print()

    visible = lines[: args.max_lines]
    print("\n".join(visible))
    if len(lines) > args.max_lines:
        print()
        print(f"... ({len(lines) - args.max_lines} more lines)")


if __name__ == "__main__":
    main()
