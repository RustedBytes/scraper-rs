"""Example demonstrating the asyncio module usage."""

import asyncio

from scraper_rs import asyncio as async_scraper

html = """
<html>
  <body>
    <div class="item" data-id="1"><a href="/a">First</a></div>
    <div class="item" data-id="2"><a href="/b">Second</a></div>
    <div class="item" data-id="3"><a href="/c">Third</a></div>
  </body>
</html>
"""


async def main():
    # Example 1: Parse HTML asynchronously
    print("Example 1: Async parse")
    doc = await async_scraper.parse(html)
    print(f"  Document text: {doc.text}")
    print()

    # Example 2: Select elements by CSS selector asynchronously
    print("Example 2: Async select")
    items = await async_scraper.select(html, ".item")
    print(f"  Found {len(items)} items:")
    for item in items:
        print(f"    - {item.tag}: {item.text} (data-id={item.attr('data-id')})")
    print()

    # Example 3: Select elements by XPath asynchronously
    print("Example 3: Async xpath")
    links = await async_scraper.xpath(html, "//a[@href]")
    print(f"  Found {len(links)} links:")
    for link in links:
        print(f"    - {link.text}: {link.attr('href')}")
    print()

    # Example 3b: Select first element by CSS selector asynchronously
    print("Example 3b: Async select_first")
    first_item = await async_scraper.select_first(html, ".item")
    if first_item:
        print(f"  First item: {first_item.text} (data-id={first_item.attr('data-id')})")
    print()

    # Example 3c: Select first element by XPath asynchronously
    print("Example 3c: Async xpath_first")
    first_link = await async_scraper.xpath_first(html, "//a[@href]")
    if first_link:
        print(f"  First link: {first_link.text} -> {first_link.attr('href')}")
    print()

    # Example 4: Multiple concurrent async operations
    print("Example 4: Concurrent async operations")
    results = await asyncio.gather(
        async_scraper.select(html, ".item"),
        async_scraper.xpath(html, "//a"),
        async_scraper.parse(html),
    )
    items, links, doc = results
    print(f"  Selected {len(items)} items and {len(links)} links concurrently")
    print(f"  Document text: {doc.text}")
    print()

    # Example 5: Using with size limits
    print("Example 5: Using with size limits")
    try:
        # This will work with sufficient size
        ok_limit = len(html.encode("utf-8"))
        items = await async_scraper.select(html, ".item", max_size_bytes=ok_limit)
        print(f"  ✓ Selected {len(items)} items with size limit {ok_limit}")

        # This will fail with a tiny limit
        tiny_limit = 10
        try:
            await async_scraper.select(html, ".item", max_size_bytes=tiny_limit)
        except ValueError as e:
            print(f"  ✓ Correctly raised error with tiny limit: {str(e)[:50]}...")
    except Exception as e:
        print(f"  Error: {e}")


if __name__ == "__main__":
    asyncio.run(main())
