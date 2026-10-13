"""parity-technical (Port 8903): Canonicals, Directives, Hreflang, Links, Sitemaps."""

from lib import *

PORT = 8903


def xml_urlset(urls) -> str:
    out = '<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
    for u in urls:
        out += f"  <url><loc>{esc(u)}</loc></url>\n"
    return out + "</urlset>\n"


def build() -> Site:
    s = Site("parity-technical", PORT)
    A = s.internal_abs
    hubs = []
    s.file("/img/small.png", png(color=(40, 120, 200)))

    # ------------------------------------------------------------------ Canonicals
    P = "/canon/"
    items = []

    def cp(name, **kw):
        path = P + name + ".html"
        std(s, path, "canon " + name, parent=P, p_words=150, **kw)
        items.append((path, name))
        return path

    c_self = cp("self", canonical=["/canon/self.html"])
    c_target = cp("target", canonical=["/canon/target.html"])
    c_none = cp("none")
    c_other = cp("to-other", canonical=["/canon/target.html"])
    c_ext = cp("to-external", canonical=[s.ext("/canon/target.html")])
    c_same = cp("multi-same", canonical=["/canon/multi-same.html", "/canon/multi-same.html"])
    c_conf = cp("multi-conflict", canonical=["/canon/target.html", "/canon/self.html"])
    c_404 = cp("to-404", canonical=["/canon/gone-404.html"])
    c_301 = cp("to-301", canonical=["/canon/moved-301"])
    c_noidx = cp("to-noindex", canonical=["/canon/noindex-target.html"])
    cp("noindex-target", robots=["noindex, follow"])
    c_hdr = cp("header-only")
    s.route("/canon/header-only.html", headers={"Link": '</canon/header-only.html>; rel="canonical"'})
    c_mixed = cp("multi-mixed-same", canonical=["/canon/multi-mixed-same.html"])
    s.route("/canon/multi-mixed-same.html", headers={"Link": '</canon/multi-mixed-same.html>; rel="canonical"'})
    s.route("/canon/moved-301", status=301, location="/canon/target.html")
    s.route(
        "/canon/gone-404.html",
        status=404,
        body=simple_html("Canonical target gone"),
    )
    hub(s, P, "canon hub", items)
    hubs.append((P, "Canonicals"))

    s.case("Canonicals", "All", hit=[c_self, c_none], miss=["/canon/gone-404.html", "/img/small.png"])
    s.case("Canonicals", "Contains Canonical", hit=[c_self, c_hdr], miss=[c_none])
    s.case("Canonicals", "Self Referencing", hit=[c_self, c_target, c_hdr, c_same], miss=[c_other, c_none])
    s.case("Canonicals", "Canonicalised", hit=[c_other, c_ext, c_404], miss=[c_self, c_none])
    s.case("Canonicals", "Missing", hit=[c_none], miss=[c_self, c_hdr])
    s.case("Canonicals", "Multiple", hit=[c_same, c_conf, c_mixed], miss=[c_self, c_none])
    s.case("Canonicals", "Multiple Conflicting", hit=[c_conf], miss=[c_same, c_mixed, c_self])
    s.case("Canonicals", "Non-Indexable Canonical", hit=[c_404, c_301, c_noidx], miss=[c_other, c_self])

    # ------------------------------------------------------------------ Directives
    P = "/dir/"
    items = []

    def dp(name, **kw):
        path = P + name + ".html"
        std(s, path, "dir " + name, parent=P, p_words=150, **kw)
        items.append((path, name))
        return path

    d_plain = dp("plain")
    d_idx = dp("index-follow", robots=["index, follow"])
    d_all = dp("all", robots=["all"])
    d_noidx = dp("noindex", robots=["noindex, follow"])
    d_hdr = dp("noindex-header")
    s.route("/dir/noindex-header.html", headers={"X-Robots-Tag": "noindex"})
    d_nofollow = dp(
        "nofollow",
        robots=["index, nofollow"],
        body_extra='<p><a href="/dir/only-via-nofollow.html">Only linked from a nofollow page</a></p>\n',
    )
    dp("only-via-nofollow")
    d_none = dp("none", robots=["none"])
    d_conf = dp("conflict", robots=["index", "noindex"])
    r_target = dp("refresh-target")
    d_ref0 = dp("refresh-0", head_extra=f'<meta http-equiv="refresh" content="0; url={r_target}">\n')
    d_refbare = dp("refresh-bare", head_extra='<meta http-equiv="refresh" content="30">\n')
    d_refhdr = dp("refresh-header")
    s.route("/dir/refresh-header.html", headers={"Refresh": f"0; url={r_target}"})
    hub(s, P, "dir hub", items)
    hubs.append((P, "Directives"))

    s.case("Directives", "All", hit=[d_plain, d_noidx], miss=["ext:/dir/plain.html"])
    s.case("Directives", "Index", hit=[d_idx, d_conf], miss=[d_plain, d_all])
    s.case("Directives", "Noindex", hit=[d_noidx, d_hdr, d_conf], miss=[d_plain, d_none])
    s.case("Directives", "Follow", hit=[d_idx, d_noidx], miss=[d_plain, d_nofollow])
    s.case("Directives", "Nofollow", hit=[d_nofollow], miss=[d_idx, d_none])
    s.case("Directives", "None", hit=[d_none], miss=[d_noidx, d_plain])
    s.case("Directives", "Refresh", hit=[d_ref0, d_refbare, d_refhdr], miss=[d_plain, r_target])

    # ------------------------------------------------------------------ Hreflang
    P = "/hl/"
    items = []
    en_codes = [
        ("en", "/hl/en/"),
        ("de", "/hl/de/"),
        ("fr", "/hl/fr/"),
        ("es", "/hl/es-old/"),
        ("it", "/hl/it/"),
        ("x-default", "/hl/en/"),
    ]
    std(s, "/hl/en/", "hl en", parent=P, p_words=150, hreflang=en_codes)
    std(s, "/hl/de/", "hl de", parent=P, p_words=150, hreflang=[("de", "/hl/de/"), ("en", "/hl/en/")])
    std(s, "/hl/fr/", "hl fr", parent=P, p_words=150)
    std(s, "/hl/es/", "hl es", parent=P, p_words=150)
    std(s, "/hl/about.html", "hl about", parent=P, p_words=150)
    std(
        s,
        "/hl/bad-codes.html",
        "hl bad codes",
        parent=P,
        p_words=150,
        hreflang=[("en-GB", "/hl/en/"), ("en-UK", "/hl/en/"), ("en_GB", "/hl/en/"), ("eng", "/hl/en/"), ("jp", "/hl/en/")],
    )
    s.route("/hl/es-old/", status=301, location="/hl/es/")
    s.route("/hl/it/", status=404, body=simple_html("Italian page missing"))
    items = [
        ("/hl/en/", "en"),
        ("/hl/de/", "de"),
        ("/hl/fr/", "fr"),
        ("/hl/es-old/", "es-old (301)"),
        ("/hl/about.html", "about"),
        ("/hl/bad-codes.html", "bad codes"),
    ]
    hub(s, P, "hl hub", items)
    hubs.append((P, "Hreflang"))

    s.case("Hreflang", "All", hit=["/hl/en/", "/hl/fr/"], miss=["/hl/it/", "/img/small.png"])
    s.case("Hreflang", "Contains Hreflang", hit=["/hl/en/", "/hl/de/", "/hl/bad-codes.html"], miss=["/hl/fr/", "/hl/about.html"])
    s.case("Hreflang", "Non-200 Hreflang URLs", hit=["/hl/en/"], miss=["/hl/de/"])
    s.case("Hreflang", "Missing Return Links", hit=["/hl/en/"], miss=["/hl/de/"])
    s.case("Hreflang", "Incorrect Language & Region Codes", hit=["/hl/bad-codes.html"], miss=["/hl/en/", "/hl/de/"])
    s.case("Hreflang", "Missing", hit=["/hl/fr/", "/hl/about.html"], miss=["/hl/en/", "/hl/bad-codes.html"])

    # ------------------------------------------------------------------ Links
    P = "/links/"
    chain = [f"/links/l{i}.html" for i in range(1, 6)]
    for i, path in enumerate(chain):
        extra = f'<p><a href="{chain[i + 1]}">Next page in the chain</a></p>\n' if i + 1 < len(chain) else ""
        std(s, path, f"links l{i + 1}", parent=P, p_words=100, body_extra=extra)
    for t in "abcdefgh":
        std(s, f"/links/target-{t}.html", f"links target {t}", parent=P, p_words=100)
    std(s, "/links/deadend.html", "links deadend", nav=False, p_words=100, footer=None)
    std(
        s,
        "/links/self-only.html",
        "links self only",
        nav=False,
        p_words=100,
        footer=None,
        body_extra='<p><a href="/links/self-only.html">This page</a></p>\n',
    )
    std(
        s,
        "/links/attrs.html",
        "links attrs",
        parent=P,
        p_words=100,
        body_extra=(
            '<p><a href="/links/target-a.html" rel="nofollow">Nofollow link</a>\n'
            '<a href="/links/target-b.html" rel="ugc">UGC link</a>\n'
            '<a href="/links/target-c.html" rel="sponsored">Sponsored link</a>\n'
            '<a href="/links/target-d.html" rel="noopener">Noopener link</a>\n'
            '<a href="/links/target-e.html">Normal link</a></p>\n'
        ),
    )
    std(
        s,
        "/links/no-anchor.html",
        "links no anchor",
        parent=P,
        p_words=100,
        body_extra=(
            '<p><a href="/links/target-f.html"></a>\n'
            '<a href="/links/target-g.html"><img src="/img/small.png" alt=""></a>\n'
            '<a href="/links/target-h.html"><img src="/img/small.png" alt="Logo link"></a></p>\n'
        ),
    )
    std(
        s,
        "/links/multi.html",
        "links multi",
        parent=P,
        p_words=100,
        body_extra=(
            '<p><a href="/links/target-a.html">First link to a</a>\n'
            '<a href="/links/target-a.html">Second link to a</a>\n'
            '<a href="/links/target-a.html">Third link to a</a>\n'
            f'<a href="{s.ext("/links/target-a.html")}">External link one</a>\n'
            f'<a href="{s.ext("/links/target-b.html")}">External link two</a></p>\n'
        ),
    )
    link_items = [(chain[0], "chain start"), ("/links/deadend.html", "dead end"), ("/links/self-only.html", "self only"),
                  ("/links/attrs.html", "attributes"), ("/links/no-anchor.html", "no anchor text"), ("/links/multi.html", "multiple links")]
    link_items += [(f"/links/target-{t}.html", f"target {t}") for t in "abcdefgh"]
    hub(s, P, "links hub", link_items)
    hubs.append((P, "Links"))

    s.case("Links", "All", hit=[chain[0], "/links/attrs.html"], miss=["/img/small.png"])
    s.case("Links", "Pages With High Crawl Depth", hit=[chain[2], chain[3], chain[4]], miss=[chain[0], chain[1], "/links/attrs.html"])
    s.case("Links", "Pages Without Internal Outlinks", hit=["/links/deadend.html", "/links/self-only.html"], miss=["/links/attrs.html", chain[0]])
    s.case("Links", "Internal Nofollow Outlinks", hit=["/links/attrs.html"], miss=[chain[0], "/links/deadend.html", "/links/multi.html"])
    s.case("Links", "Internal Outlinks With No Anchor Text", hit=["/links/no-anchor.html"], miss=["/links/attrs.html", chain[0]])

    # ------------------------------------------------------------------ Baum (Linkstruktur)
    P = "/tree/"
    cats = []
    for c in range(1, 7):
        kids = [f"/tree/c{c}/p{j}.html" for j in range(1, 7)]
        sib = f"/tree/c{c % 6 + 1}/"
        std(
            s,
            f"/tree/c{c}/",
            f"tree category {c}",
            parent=P,
            p_words=80,
            body_extra=link_list([(k, f"Page {k.rsplit('/', 1)[1]}") for k in kids]) + f'<p><a href="{sib}">Next category</a></p>\n',
        )
        for j, k in enumerate(kids):
            nxt = kids[(j + 1) % len(kids)]
            std(s, k, f"tree c{c} p{j + 1}", parent=f"/tree/c{c}/", p_words=120,
                body_extra=f'<p><a href="{nxt}">Related page</a></p>\n')
        cats.append((f"/tree/c{c}/", f"Category {c}"))
    hub(s, P, "tree hub", cats)
    hubs.append((P, "Tree"))

    # ------------------------------------------------------------------ Sitemaps
    P = "/map/"
    items = []

    def mp(name, **kw):
        path = P + name + ".html"
        std(s, path, "map " + name, parent=P, p_words=150, **kw)
        return path

    m_in = mp("in-sitemap")
    m_both = mp("linked-and-listed")
    m_not = mp("not-in-sitemap")
    m_noidx = mp("noindex", robots=["noindex, follow"])
    m_canon = mp("canonicalised", canonical=["/map/in-sitemap.html"])
    m_p1 = mp("post-1")
    m_p2 = mp("post-2")
    m_orph = mp("orphan-page", body_extra='<p><a href="/map/orphan-child.html">Only linked from the orphan page</a></p>\n')
    m_child = mp("orphan-child", body_extra='<p><a href="/map/orphan-grandchild.html">Only linked from the orphan child</a></p>\n')
    m_grand = mp("orphan-grandchild")
    std(s, "/blocked/page.html", "blocked page", parent=P, p_words=100)
    s.route("/map/redirected", status=301, location="/map/in-sitemap.html")
    s.route("/map/gone", status=410, body=simple_html("Gone"))
    for p_, t in ((m_in, "in sitemap"), (m_both, "listed and linked"), (m_not, "not in sitemap"), (m_noidx, "noindex"),
                  (m_canon, "canonicalised"), (m_p1, "post 1"), (m_p2, "post 2"), ("/blocked/page.html", "blocked by robots.txt")):
        items.append((p_, t))
    hub(s, P, "map hub", items)
    hubs.append((P, "Sitemaps"))

    s.file("/robots.txt", f"User-agent: *\nDisallow: /blocked/\n\nSitemap: {A('/sitemap_index.xml')}\n")
    s.file(
        "/sitemap_index.xml",
        '<?xml version="1.0" encoding="UTF-8"?>\n<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
        f"  <sitemap><loc>{A('/sitemap-pages.xml')}</loc></sitemap>\n"
        f"  <sitemap><loc>{A('/sitemap-posts.xml.gz')}</loc></sitemap>\n"
        "</sitemapindex>\n",
    )
    s.file(
        "/sitemap-pages.xml",
        xml_urlset(
            [A(p_) for p_ in (m_in, m_both, "/map/redirected", "/map/gone", m_noidx, m_canon, "/blocked/page.html", m_orph)]
        ),
    )
    s.file("/sitemap-posts.xml.gz", gz(xml_urlset([A(p_) for p_ in (m_p1, m_p2, m_both)]).encode("utf-8")))
    s.route("/sitemap-posts.xml.gz", content_type="application/gzip")

    s.case("Sitemaps", "All", hit=[m_in, m_not], miss=["ext:/map/in-sitemap.html"])
    s.case("Sitemaps", "URLs In Sitemap", hit=[m_in, m_p1, m_orph], miss=[m_not, m_child])
    s.case("Sitemaps", "URLs Not In Sitemap", hit=[m_not], miss=[m_in, m_noidx, "/img/small.png"])
    s.case("Sitemaps", "Orphan URLs", hit=[m_orph, m_child, m_grand], miss=[m_both, m_in, m_not])
    s.case(
        "Sitemaps",
        "Non-Indexable URLs In Sitemap",
        hit=["/map/redirected", "/map/gone", m_noidx, m_canon, "/blocked/page.html"],
        miss=[m_in, m_p1],
    )

    # ------------------------------------------------------------------ Startseite
    ext_refs = [(s.ext("/dir/plain.html"), "external host: plain page"), (s.ext("/map/in-sitemap.html"), "external host: page that is in the sitemap")]
    std(s, "/index.html", "parity technical home", nav=False, body_extra=link_list(hubs) + link_list(ext_refs), p_words=60)
    return s
