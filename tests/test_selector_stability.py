"""Regression tests for selector stability across realistic DOM mutations."""

from scraper_rs import Document


def generated_xpath(html: str, seed: str, *, text: str | None = None) -> str:
    doc = Document(html)
    matches = doc.select(seed)
    target = next((el for el in matches if text is None or el.text.strip() == text), None)
    assert target is not None
    selector = doc.generate_xpath_selector(target)
    assert selector is not None
    return selector


def assert_survives(html_b: str, selector: str, expected_text: str) -> None:
    matches = Document(html_b).xpath(selector)
    assert len(matches) == 1, selector
    assert matches[0].text.strip() == expected_text, selector


def test_removed_testid_prefers_semantic_ancestor() -> None:
    html_a = """
    <main>
      <section data-section="lead">
        <article><a data-testid="lead-link" href="/news/lead">Lead</a></article>
      </section>
    </main>
    """
    html_b = """
    <main>
      <section data-section="lead">
        <div class="wrapper"><article><a href="/news/lead">Lead</a></article></div>
      </section>
    </main>
    """
    selector = generated_xpath(html_a, 'section[data-section="lead"] a')
    assert "@data-section='lead'" in selector
    assert_survives(html_b, selector, "Lead")


def test_href_path_survives_query_churn() -> None:
    html_a = """
    <main>
      <a href="/news/tracked?utm_source=homepage&utm_campaign=october">Tracked</a>
      <a href="/news/other">Other</a>
    </main>
    """
    html_b = """
    <main>
      <a href="/news/tracked?utm_source=telegram&utm_campaign=november">Tracked</a>
      <a href="/news/other">Other</a>
    </main>
    """
    selector = generated_xpath(html_a, "a", text="Tracked")
    assert "ends-with(" in selector
    assert "/news/tracked" in selector
    assert_survives(html_b, selector, "Tracked")


def test_href_path_survives_relative_to_absolute_url() -> None:
    html_a = """
    <main>
      <a href="/news/absolute">Article</a>
      <a href="/news/other">Other</a>
    </main>
    """
    html_b = """
    <main>
      <a href="https://example.com/news/absolute">Article</a>
      <a href="/news/other">Other</a>
    </main>
    """
    selector = generated_xpath(html_a, "a", text="Article")
    assert "ends-with(" in selector
    assert_survives(html_b, selector, "Article")


def test_generated_id_is_penalized_below_semantic_ancestor() -> None:
    html_a = """
    <main>
      <section data-section="generated-id">
        <article id="article-839271"><h2>Generated ID article</h2></article>
      </section>
    </main>
    """
    html_b = """
    <main>
      <section data-section="generated-id">
        <article id="article-194552"><div><h2>Generated ID article</h2></div></article>
      </section>
    </main>
    """
    selector = generated_xpath(
        html_a, 'section[data-section="generated-id"] article'
    )
    assert "article-839271" not in selector
    assert "@data-section='generated-id'" in selector
    assert_survives(html_b, selector, "Generated ID article")


def test_text_predicate_beats_positional_xpath_for_attrless_target() -> None:
    html_a = """
    <section data-section="no-attrs">
      <div><p>A</p><p>Target plain paragraph</p><p>C</p></div>
    </section>
    """
    html_b = """
    <section data-section="no-attrs">
      <div class="outer"><div>
        <p>Inserted</p><p>A</p><p>Target plain paragraph</p><p>C</p>
      </div></div>
    </section>
    """
    selector = generated_xpath(
        html_a, 'section[data-section="no-attrs"] p', text="Target plain paragraph"
    )
    assert "normalize-space(.)" in selector
    assert_survives(html_b, selector, "Target plain paragraph")


def test_far_semantic_ancestor_beats_near_presentation_class() -> None:
    html_a = """
    <main>
      <section data-testid="ancestor-stable">
        <div class="content-grid"><div class="content-cell"><span>Target</span></div></div>
      </section>
      <span>Decoy</span>
    </main>
    """
    html_b = """
    <main>
      <section data-testid="ancestor-stable">
        <div class="grid css-module__grid__91a">
          <div class="cell css-module__cell__72q"><div><span>Target</span></div></div>
        </div>
      </section>
      <span>Decoy</span>
    </main>
    """
    selector = generated_xpath(
        html_a, 'section[data-testid="ancestor-stable"] span'
    )
    assert "@data-testid='ancestor-stable'" in selector
    assert "content-cell" not in selector
    assert_survives(html_b, selector, "Target")


def test_semantic_ancestor_beats_class_hash_churn() -> None:
    html_a = """
    <main>
      <section data-section="class-hash">
        <article class="card css-module__card__aa11">
          <span class="title css-module__title__bb22">Hashed class target</span>
        </article>
      </section>
    </main>
    """
    html_b = """
    <main>
      <section data-section="class-hash">
        <article class="story css-module__card__zz91">
          <div><span class="headline css-module__title__yy82">Hashed class target</span></div>
        </article>
      </section>
    </main>
    """
    selector = generated_xpath(
        html_a, 'section[data-section="class-hash"] span'
    )
    assert "@data-section='class-hash'" in selector
    assert "css-module" not in selector
    assert_survives(html_b, selector, "Hashed class target")
