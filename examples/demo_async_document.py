"""Example showing the async document-oriented API."""

from __future__ import annotations

import asyncio

from scraper_rs import asyncio as async_scraper

HTML = """
<html>
  <body>
    <main>
      <section class="card" data-id="1">
        <h2>Alpha</h2>
        <a href="/alpha">Read more</a>
      </section>
      <section class="card featured" data-id="2">
        <h2>Beta</h2>
        <a href="/beta">Read more</a>
      </section>
      <section class="card" data-id="3">
        <h2>Gamma</h2>
        <a href="/gamma">Read more</a>
      </section>
    </main>
  </body>
</html>
"""


async def main() -> None:
    print("AsyncDocument workflow")
    print()

    async with await async_scraper.parse(HTML) as doc:
        print(doc)
        print(f"Document text: {doc.text}")
        print()

        cards = await doc.select(".card")
        print(f"Found {len(cards)} cards")

        for card in cards:
            title = await card.select_first("h2")
            link = await card.select_first("a[href]")
            print(
                f"  - id={card.attr('data-id')}: "
                f"{title.text if title else 'missing title'} "
                f"-> {link.attr('href') if link else 'missing link'}"
            )

        print()

        featured = await doc.find(".featured")
        if featured:
            featured_link = await featured.find("a[href]")
            print("Featured card")
            print(f"  text: {featured.text}")
            print(
                f"  href: {featured_link.attr('href') if featured_link else 'missing'}"
            )
            print()

        first_link, first_featured = await asyncio.gather(
            doc.select_first("a[href]"),
            doc.xpath_first("//section[contains(@class, 'featured')]"),
        )
        print("Concurrent queries")
        print(f"  first link: {first_link.attr('href') if first_link else 'missing'}")
        print(
            "  featured via xpath: "
            f"{first_featured.attr('data-id') if first_featured else 'missing'}"
        )
        print()

        pretty = await doc.prettify()
        print("Prettified HTML preview")
        for line in pretty.splitlines()[:10]:
            print(f"  {line}")


if __name__ == "__main__":
    asyncio.run(main())
