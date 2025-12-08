from scraper_rs import Document, first, select, select_first, xpath

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

print(doc)  # <Document len_html=...>
print(doc.text)  # "First Second"

items = doc.select(".item")
for el in items:
    print(el.tag)  # "div"
    print(el.text)  # "First" / "Second"
    print(el.attr("data-id"))
    print(el.attrs)  # full attribute dict
    print(el.to_dict())  # handy for debugging / serialization

    nested_link = el.select_first("a[href]")
    if nested_link:
        print(nested_link.attr("href"))

first_link = doc.select_first("a[href]")  # alias: doc.find(...)
if first_link:
    print(first_link.text, first_link.attr("href"))

# 2) Functional “one-shot” helpers:
links = select(html, "a[href]")
print(links)  # [Element(...), Element(...)]

first_link = first(html, "a[href]")
print(first_link)
print(select_first(html, "a[href]"))

# 3) XPath helpers (expressions must return element nodes):
xpath_links = doc.xpath("//div[@class='item']/a")
print([link.text for link in xpath_links])  # ["First", "Second"]

first_xpath = doc.xpath_first("//div[@data-id='2']/a")
if first_xpath:
    print(first_xpath.text, first_xpath.attr("href"))

print([link.text for link in xpath(html, "//div[@class='item']/a")])
