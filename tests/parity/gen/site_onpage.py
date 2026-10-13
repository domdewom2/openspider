"""parity-onpage (Port 8902): Titel, Meta Description, H1, H2, Content, Images, URL, Inhaltstypen."""

from lib import *

PORT = 8902


def build() -> Site:
    s = Site("parity-onpage", PORT)
    s.file("/robots.txt", "User-agent: *\nDisallow:\n")
    hubs = []  # (href, text)

    # ------------------------------------------------------------------ Titles
    P = "/titles/"
    items = []

    def tp(name, **kw):
        std(s, P + name + ".html", "titles " + name, parent=P, **kw)
        items.append((P + name + ".html", name))
        return P + name + ".html"

    ok1 = tp("ok-1")
    tp("ok-2")
    miss_none = tp("missing-none", title=None)
    miss_empty = tp("missing-empty", title="")
    miss_space = tp("missing-spaces", title="   ")
    len60 = tp("len-60", title=exact_text("t60", 60, "Sixty"))
    len61 = tp("len-61", title=exact_text("t61", 61, "Sixtyone"))
    len29 = tp("len-29", title=exact_text("t29", 29, "Twentynine"))
    len30 = tp("len-30", title=exact_text("t30", 30, "Thirty"))
    padded = tp("len-58-padded", title="     " + exact_text("t58", 58, "Padded") + "     ")
    multi = tp("multi-two", title=[fit_text("m1", 34, 50, "First title"), fit_text("m2", 34, 50, "Second title")])
    dup_title = "Duplicate title fixture shared by two pages"
    dup_a = tp("dup-a", title=dup_title)
    dup_b = tp("dup-b", title=dup_title)
    dup_case = tp("dup-case", title=dup_title.lower())
    dup_canon = tp("dup-canonical", title=dup_title, canonical=["/titles/dup-a.html"])
    px_w = tp("pixel-W60", title="W" * 60)
    px_i = tp("pixel-i70", title="i" * 70)
    hub(s, P, "titles hub", items)
    hubs.append((P, "Page Titles"))

    s.case("Page Titles", "All", hit=[ok1, miss_none], miss=["/img/small.png"])
    s.case("Page Titles", "Missing", hit=[miss_none, miss_empty, miss_space], miss=[ok1, multi, len30])
    s.case("Page Titles", "Duplicate", hit=[dup_a, dup_b], miss=[dup_case, dup_canon, ok1])
    s.case("Page Titles", "Over 60 Characters", hit=[len61, px_i], miss=[len60, padded, ok1])
    s.case("Page Titles", "Below 30 Characters", hit=[len29], miss=[len30, miss_none, ok1])
    s.case("Page Titles", "Multiple", hit=[multi], miss=[ok1, len60])

    # ------------------------------------------------------------------ Meta Description
    P = "/meta/"
    items = []

    def mp(name, **kw):
        std(s, P + name + ".html", "meta " + name, parent=P, **kw)
        items.append((P + name + ".html", name))
        return P + name + ".html"

    ok1 = mp("ok-1")
    mp("ok-2")
    m_none = mp("missing-none", desc=None)
    m_empty = mp("missing-empty", desc="")
    m_og = mp(
        "og-only",
        desc=None,
        head_extra='<meta property="og:description" content="Only an Open Graph description, no name=description.">\n',
    )
    l155 = mp("len-155", desc=exact_text("d155", 155, "Hundred fifty five"))
    l156 = mp("len-156", desc=exact_text("d156", 156, "Hundred fifty six"))
    l69 = mp("len-69", desc=exact_text("d69", 69, "Sixty nine"))
    l70 = mp("len-70", desc=exact_text("d70", 70, "Seventy"))
    m_multi = mp("multi-two", desc=[fit_text("dm1", 80, 120, "First description"), fit_text("dm2", 80, 120, "Second description")])
    dd = fit_text("ddup", 90, 120, "Duplicate description shared by two pages")
    d_a = mp("dup-a", desc=dd)
    d_b = mp("dup-b", desc=dd)
    d_case = mp("dup-case", desc=dd.upper())
    d_canon = mp("dup-canonical", desc=dd, canonical=["/meta/dup-a.html"])
    hub(s, P, "meta hub", items)
    hubs.append((P, "Meta Description"))

    s.case("Meta Description", "All", hit=[ok1, m_none], miss=["/img/small.png"])
    s.case("Meta Description", "Missing", hit=[m_none, m_empty, m_og], miss=[ok1, m_multi, l70])
    s.case("Meta Description", "Duplicate", hit=[d_a, d_b], miss=[d_case, d_canon, ok1])
    s.case("Meta Description", "Over 155 Characters", hit=[l156], miss=[l155, ok1])
    s.case("Meta Description", "Below 70 Characters", hit=[l69], miss=[l70, m_none, ok1])
    s.case("Meta Description", "Multiple", hit=[m_multi], miss=[ok1])

    # ------------------------------------------------------------------ H1
    P = "/h1/"
    items = []

    def hp(name, **kw):
        std(s, P + name + ".html", "h1 " + name, parent=P, **kw)
        items.append((P + name + ".html", name))
        return P + name + ".html"

    ok1 = hp("ok-1")
    h_none = hp("missing-none", h1=None)
    h_empty = hp("missing-empty", h1="")
    h70 = hp("len-70", h1=exact_text("h70", 70, "Seventy"))
    h71 = hp("len-71", h1=exact_text("h71", 71, "Seventyone"))
    h_multi = hp("multi-two", h1=[fit_text("hm1", 20, 40, "First heading"), fit_text("hm2", 20, 40, "Second heading")])
    h_five = hp("five-h1", h1=[fit_text(f"hf{i}", 20, 40, f"Heading number {i}") for i in range(5)])
    hd = "Duplicate heading shared by two pages"
    h_a = hp("dup-a", h1=hd)
    h_b = hp("dup-b", h1=hd)
    hub(s, P, "h1 hub", items)
    hubs.append((P, "H1"))

    s.case("H1", "All", hit=[ok1, h_none], miss=["/img/small.png"])
    s.case("H1", "Missing", hit=[h_none, h_empty], miss=[ok1, h_multi])
    s.case("H1", "Duplicate", hit=[h_a, h_b], miss=[ok1, h70])
    s.case("H1", "Over 70 Characters", hit=[h71], miss=[h70, ok1])
    s.case("H1", "Multiple", hit=[h_multi, h_five], miss=[ok1, h_none])

    # ------------------------------------------------------------------ H2
    P = "/h2/"
    items = []

    def h2p(name, **kw):
        std(s, P + name + ".html", "h2 " + name, parent=P, **kw)
        items.append((P + name + ".html", name))
        return P + name + ".html"

    ok1 = h2p("ok-1")
    s_none = h2p("missing-none", h2=None)
    s_empty = h2p("missing-empty", h2=" ")
    s70 = h2p("len-70", h2=exact_text("s70", 70, "Seventy"))
    s71 = h2p("len-71", h2=exact_text("s71", 71, "Seventyone"))
    s_multi = h2p("multi-two", h2=[fit_text("sm1", 20, 40, "First section"), fit_text("sm2", 20, 40, "Second section")])
    s_twelve = h2p("twelve", h2=[fit_text(f"st{i}", 20, 40, f"Section number {i}") for i in range(12)])
    sd = "Duplicate section shared by two pages"
    s_a = h2p("dup-a", h2=sd)
    s_b = h2p("dup-b", h2=sd)
    hub(s, P, "h2 hub", items)
    hubs.append((P, "H2"))

    s.case("H2", "All", hit=[ok1, s_none], miss=["/img/small.png"])
    s.case("H2", "Missing", hit=[s_none, s_empty], miss=[ok1, s_multi])
    s.case("H2", "Duplicate", hit=[s_a, s_b], miss=[ok1, s70])
    s.case("H2", "Over 70 Characters", hit=[s71], miss=[s70, ok1])
    s.case("H2", "Multiple", hit=[s_multi, s_twelve], miss=[ok1, s_none])

    # ------------------------------------------------------------------ Content
    P = "/content/"
    items = []

    def cp(name, **kw):
        path = P + name + ".html"
        text = std(s, path, "content " + name, parent=P, **kw)
        items.append((path, name))
        return path, text

    def exact_words_page(name, total, **kw):
        # Inhaltsbereich = h1 + h2 + p (nav/footer sind ausgeschlossen)
        h1 = fit_text("cw1" + name, 20, 40, "Heading " + name.replace("-", " "))
        h2 = fit_text("cw2" + name, 20, 40, "Section " + name.replace("-", " "))
        p_words = total - count_words(h1) - count_words(h2)
        return cp(name, h1=h1, h2=h2, p_words=p_words, **kw)

    w199, _ = exact_words_page("words-199", 199)
    w200, _ = exact_words_page("words-200", 200)
    short50, _ = exact_words_page("short-50", 50)
    noidx50, _ = exact_words_page("noindex-50", 50, robots=["noindex, follow"])
    # footer-heavy: 150 Woerter im Inhaltsbereich, 300 im Footer
    h1 = fit_text("cwf1", 20, 40, "Heading footer heavy")
    h2 = fit_text("cwf2", 20, 40, "Section footer heavy")
    foot, _ = cp(
        "footer-heavy",
        h1=h1,
        h2=h2,
        p_words=150 - count_words(h1) - count_words(h2),
        footer=lorem("footer-heavy", 300),
    )
    # exakte Duplikate: identische Bytes unter drei Pfaden (+ Query-Variante, + noindex-Variante per Header)
    dup_text = std(s, "/content/dupe-a.html", "content dupe", parent=P, store=False)
    s.file("/content/dupe-a.html", dup_text)
    s.file("/content/dupe-b.html", dup_text)
    s.file("/content/dupe-c.html", dup_text.replace("</p>", " </p>", 1))
    s.file("/content/dupe-noindex.html", dup_text)
    s.route("/content/dupe-noindex.html", headers={"X-Robots-Tag": "noindex"})
    items += [
        ("/content/dupe-a.html", "dupe-a"),
        ("/content/dupe-b.html", "dupe-b"),
        ("/content/dupe-c.html", "dupe-c (plus one space)"),
        ("/content/dupe-a.html?ref=x", "dupe-a with query"),
        ("/content/dupe-noindex.html", "dupe-noindex"),
    ]
    hub(s, P, "content hub", items)
    hubs.append((P, "Content"))

    s.case("Content", "All", hit=[w200, foot], miss=["/img/small.png"])
    s.case(
        "Content",
        "Exact Duplicates",
        hit=["/content/dupe-a.html", "/content/dupe-b.html", "/content/dupe-a.html?ref=x"],
        miss=["/content/dupe-c.html", "/content/dupe-noindex.html", w199],
    )
    s.case(
        "Content",
        "Low Content Pages",
        hit=[w199, foot, short50],
        miss=[w200, noidx50, "/content/dupe-a.html"],
    )

    # ------------------------------------------------------------------ Images
    P = "/images/"
    s.file("/img/small.png", png(color=(40, 120, 200)))
    s.file("/img/size-102400.png", png(color=(10, 10, 10), exact_size=102400))
    s.file("/img/size-102401.png", png(color=(20, 20, 20), exact_size=102401))
    s.file("/img/empty-alt.png", png(color=(60, 130, 60)))
    s.file("/img/no-alt.png", png(color=(130, 60, 60)))
    s.file("/img/alt-100.png", png(color=(90, 90, 160)))
    s.file("/img/alt-101.png", png(color=(160, 90, 90)))
    s.file("/img/good-alt.png", png(color=(90, 160, 90)))
    alt100 = exact_text("alt100", 100, "Alt text with one hundred characters")
    alt101 = exact_text("alt101", 101, "Alt text with one hundred and one characters")
    gallery = (
        '<img src="/img/small.png" alt="Small image">\n'
        '<img src="/img/size-102400.png" alt="Exactly 102400 bytes">\n'
        '<img src="/img/size-102401.png" alt="One byte more than 102400">\n'
        f'<img src="{s.ext("/img/size-102401.png")}" alt="Same big image on the external host">\n'
        '<img src="/img/missing.png" alt="Image that does not exist">\n'
    )
    alts = (
        '<img src="/img/good-alt.png" alt="A good alternative text">\n'
        '<img src="/img/empty-alt.png" alt="">\n'
        '<img src="/img/no-alt.png">\n'
        f'<img src="/img/alt-100.png" alt="{esc(alt100)}">\n'
        f'<img src="/img/alt-101.png" alt="{esc(alt101)}">\n'
    )
    std(s, "/images/gallery.html", "images gallery", parent=P, body_extra=gallery, p_words=60)
    std(s, "/images/alt.html", "images alt", parent=P, body_extra=alts, p_words=60)
    hub(s, P, "images hub", [("/images/gallery.html", "gallery"), ("/images/alt.html", "alt texts")])
    hubs.append((P, "Images"))

    s.case("Images", "All", hit=["/img/small.png", "/img/missing.png"], miss=["/images/gallery.html"])
    s.case(
        "Images",
        "Over 100 KB",
        hit=["/img/size-102401.png", "ext:/img/size-102401.png"],
        miss=["/img/size-102400.png", "/img/small.png"],
    )
    s.case("Images", "Missing Alt Text", hit=["/img/empty-alt.png"], miss=["/img/good-alt.png", "/img/no-alt.png"])
    s.case("Images", "Missing Alt Attribute", hit=["/img/no-alt.png"], miss=["/img/good-alt.png", "/img/empty-alt.png"])
    s.case(
        "Images",
        "Alt Text Over 100 Characters",
        hit=["/img/alt-101.png"],
        miss=["/img/alt-100.png", "/img/good-alt.png"],
    )

    # ------------------------------------------------------------------ URL
    P = "/urls/"
    base_len = len(s.internal_abs(""))  # "http://127.0.0.1:8902"
    long94 = "/urls/" + "lang-" + "x" * (94 - 6 - 5 - 5) + ".html"  # Pfad 94 Zeichen -> Adresse 115
    long95 = "/urls/" + "lang-" + "x" * (95 - 6 - 5 - 5) + ".html"
    assert len(long94) == 94 and len(long95) == 95
    assert base_len + len(long94) == 115 and base_len + len(long95) == 116

    def up(path, slug, **kw):
        std(s, path, "urls " + slug, parent=P, p_words=120, **kw)

    up("/urls/clean-page.html", "clean page")
    up("/urls/folder/clean-two.html", "clean two")
    up("/urls/mein_artikel.html", "underscore")
    up("/urls/Produkte.html", "uppercase")
    up("/urls/params.html", "params")
    up(long94, "long 94")
    up(long95, "long 95")
    # Routen mit rohem (prozentkodiertem bzw. doppeltem) Pfad
    for raw, slug in (
        ("/urls/%C3%BCber-uns.html", "umlaut"),
        ("/urls/a//b.html", "double slash"),
        ("/urls/mein%20artikel.html", "space"),
    ):
        text = std(s, raw, "urls " + slug, parent=P, p_words=120, store=False)
        s.route(raw, body=text)
    url_items = [
        ("/urls/clean-page.html", "clean page"),
        ("/urls/folder/clean-two.html", "clean two"),
        ("/urls/über-uns.html", "umlaut"),
        ("/urls/mein_artikel.html", "underscore"),
        ("/urls/Produkte.html", "uppercase"),
        ("/urls/a//b.html", "double slash"),
        ("/urls/mein%20artikel.html", "space"),
        ("/urls/params.html?id=1", "parameter id"),
        ("/urls/params.html?a=1&b=2", "two parameters"),
        ("/urls/params.html?sort_by=x", "parameter with underscore"),
        (long94, "address 115 characters"),
        (long95, "address 116 characters"),
    ]
    hub(s, P, "urls hub", url_items)
    hubs.append((P, "URL"))

    enc_umlaut = "/urls/%C3%BCber-uns.html"
    s.case("URL", "All", hit=["/urls/clean-page.html", enc_umlaut], miss=["ext:/ctype/t-html.html"])
    s.case("URL", "Non ASCII Characters", hit=[enc_umlaut], miss=["/urls/clean-page.html", "/urls/mein%20artikel.html"])
    s.case("URL", "Underscores", hit=["/urls/mein_artikel.html"], miss=["/urls/clean-page.html", "/urls/params.html?sort_by=x"])
    s.case("URL", "Uppercase", hit=["/urls/Produkte.html"], miss=["/urls/clean-page.html", enc_umlaut])
    s.case("URL", "Multiple Slashes", hit=["/urls/a//b.html"], miss=["/urls/clean-page.html", "/urls/params.html?id=1"])
    s.case("URL", "Contains A Space", hit=["/urls/mein%20artikel.html"], miss=["/urls/clean-page.html", "/urls/mein_artikel.html"])
    s.case(
        "URL",
        "Parameters",
        hit=["/urls/params.html?id=1", "/urls/params.html?a=1&b=2"],
        miss=["/urls/clean-page.html", "/urls/mein_artikel.html"],
    )
    s.case("URL", "Over 115 Characters", hit=[long95], miss=[long94, "/urls/clean-page.html"])

    # ------------------------------------------------------------------ Inhaltstypen intern/extern
    P = "/ctype/"
    s.file("/ctype/t-html.html", simple_html("Content type HTML") )
    s.file("/ctype/t-js.js", "// fixture script\nvar ctypeFixture = 1;\n")
    s.file("/ctype/t-css.css", "body { color: #222; }\n")
    s.file("/ctype/t-png.png", png(color=(100, 100, 100)))
    s.file("/ctype/t-svg.svg", svg("ctype"))
    s.file("/ctype/t-pdf.pdf", pdf("ctype fixture"))
    s.route("/ctype/t-pdf.pdf", content_type="application/pdf")
    s.file("/ctype/t-json.json", '{"fixture": true}\n')
    s.file("/ctype/t-xml.xml", '<?xml version="1.0"?><root><item>fixture</item></root>\n')
    s.file("/ctype/t-txt.txt", "plain text fixture\n")
    s.route(
        "/ctype/t-xhtml",
        content_type="application/xhtml+xml",
        body='<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>Content type XHTML</title></head><body><h1>XHTML</h1></body></html>',
    )
    s.route("/ctype/t-swf", content_type="application/x-shockwave-flash", body="FWS fixture")
    s.route("/ctype/t-mp4", content_type="video/mp4", body="mp4 fixture")
    s.route("/ctype/t-woff2", content_type="font/woff2", body="woff2 fixture")
    s.route("/ctype/t-invalid", content_type="foo", body="invalid content type fixture")

    internal = [
        "/ctype/t-html.html", "/ctype/t-xhtml", "/ctype/t-js.js", "/ctype/t-css.css", "/ctype/t-png.png",
        "/ctype/t-svg.svg", "/ctype/t-pdf.pdf", "/ctype/t-swf", "/ctype/t-json.json", "/ctype/t-xml.xml",
        "/ctype/t-txt.txt", "/ctype/t-mp4", "/ctype/t-woff2", "/ctype/t-invalid",
    ]
    external = [
        "/ctype/t-html.html", "/ctype/t-js.js", "/ctype/t-css.css", "/ctype/t-png.png", "/ctype/t-pdf.pdf",
        "/ctype/t-swf", "/ctype/t-json.json",
    ]
    dead = "http://localhost:1/"
    ctype_items = [(p, p.rsplit("/", 1)[1]) for p in internal]
    ctype_items += [(s.ext(p), "external " + p.rsplit("/", 1)[1]) for p in external]
    ctype_items += [(dead, "external host, closed port")]
    hub(s, P, "ctype hub", ctype_items)
    hubs.append((P, "Content types"))

    s.case("Internal", "All", hit=["/ctype/t-html.html", "/ctype/t-invalid"], miss=["ext:/ctype/t-html.html"])
    s.case("Internal", "HTML", hit=["/ctype/t-html.html", "/ctype/t-xhtml"], miss=["/ctype/t-xml.xml", "/ctype/t-js.js"])
    s.case("Internal", "JavaScript", hit=["/ctype/t-js.js"], miss=["/ctype/t-html.html", "/ctype/t-css.css"])
    s.case("Internal", "CSS", hit=["/ctype/t-css.css"], miss=["/ctype/t-js.js"])
    s.case("Internal", "Images", hit=["/ctype/t-png.png", "/ctype/t-svg.svg"], miss=["/ctype/t-pdf.pdf", "/ctype/t-html.html"])
    s.case("Internal", "PDF", hit=["/ctype/t-pdf.pdf"], miss=["/ctype/t-png.png"])
    s.case("Internal", "Unknown", hit=["/ctype/t-invalid"], miss=["/ctype/t-html.html", "/ctype/t-json.json"])
    s.case("External", "All", hit=["ext:/ctype/t-html.html", "ext:/ctype/t-json.json"], miss=["/ctype/t-html.html"])
    s.case("External", "HTML", hit=["ext:/ctype/t-html.html"], miss=["ext:/ctype/t-js.js", "/ctype/t-html.html"])
    s.case("External", "JavaScript", hit=["ext:/ctype/t-js.js"], miss=["ext:/ctype/t-css.css"])
    s.case("External", "CSS", hit=["ext:/ctype/t-css.css"], miss=["ext:/ctype/t-js.js"])
    s.case("External", "Images", hit=["ext:/ctype/t-png.png"], miss=["ext:/ctype/t-pdf.pdf"])
    s.case("External", "PDF", hit=["ext:/ctype/t-pdf.pdf"], miss=["ext:/ctype/t-png.png"])
    s.case("External", "Flash", hit=["ext:/ctype/t-swf"], miss=["ext:/ctype/t-json.json"])
    s.case("External", "Other", hit=["ext:/ctype/t-json.json"], miss=["ext:/ctype/t-html.html", "ext:/ctype/t-swf"])
    s.case("External", "Unknown", hit=["dead:" + dead], miss=["ext:/ctype/t-html.html"])

    # ------------------------------------------------------------------ Startseite
    std(
        s,
        "/index.html",
        "parity onpage home",
        nav=False,
        body_extra=link_list(hubs),
        p_words=60,
    )
    return s
