from scraper_rs import Document, select, first

html = """
<html>
  <body>
    <div class="item" data-id="1"><a href="/a">First</a></div>
    <div class="item" data-id="2"><a href="/b">Second</a></div>
  </body>
</html>
"""

# 1) Object-oriented:
doc = Document(html)

print(doc)          # <Document len_html=...>
print(doc.text)     # "First Second"

items = doc.select(".item")
for el in items:
    print(el.tag)        # "div"
    print(el.text)       # "First" / "Second"
    print(el.attr("data-id"))
    print(el.attrs)      # full attribute dict
    print(el.to_dict())  # handy for debugging / serialization

first_link = doc.find("a[href]")
if first_link:
    print(first_link.text, first_link.attr("href"))

# 2) Functional “one-shot” helpers:
links = select(html, "a[href]")
print(links)  # [Element(...), Element(...)]

first_link = first(html, "a[href]")
print(first_link)
