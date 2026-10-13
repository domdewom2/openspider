"""out-basic-site (Port 8900): Querschnittsite nach OUT-001.

12 HTML-Seiten, 2 Bilder, 1 CSS, 1 JS, ein 404, ein 301, ein externer Link.
Ausnahme von der Regel "keine Platzhalter": fuenf Dateien behalten `{{BASE_URL}}`: index.html, about.html und
products.html (Canonical), robots.txt (Sitemap-Zeile) und sitemap.xml (drei <loc>), weil die bestehenden Tests in
crates/os-testsite/tests/serve.rs diese Stellen pruefen (Platzhalter existiert seit dem Testsite-Gerüst,
ist also kein P09-Platzhalter).
"""

from lib import *


def build() -> Site:
    s = Site("out-basic-site", 8900)

    nav = (
        '<nav>\n'
        '  <a href="/about.html">About</a>\n'
        '  <a href="/products.html">Products</a>\n'
        '  <a href="/services.html">Services</a>\n'
        '  <a href="/contact.html">Contact</a>\n'
        '  <a href="/blog/">Blog</a>\n'
        '  <a href="/faq.html">FAQ</a>\n'
        '  <a href="/imprint.html">Imprint</a>\n'
        '  <a href="/old-page">Old page (redirects)</a>\n'
        '  <a href="/missing.html">Missing page (404)</a>\n'
        '</nav>\n'
    )
    small_nav = '<nav><a href="/">Home</a> <a href="/products.html">Products</a> <a href="/contact.html">Contact</a></nav>\n'

    def page(path, title, desc, h1, text, canonical=None, nav_html=small_nav, extra=""):
        head = f"<title>{esc(title)}</title>\n"
        if desc:
            head += f'<meta name="description" content="{esc(desc)}">\n'
        if canonical:
            head += f'<link rel="canonical" href="{canonical}">\n'
        head += '<link rel="stylesheet" href="/assets/site.css">\n'
        body = nav_html + f"<h1>{esc(h1)}</h1>\n<p>{esc(text)}</p>\n" + extra
        body += '<script src="/assets/site.js"></script>\n'
        s.file(path, doc(head, body))

    # 1 index.html: Startseite mit allen Abschnitten, Bild, externem Link
    page(
        "/index.html",
        "Basic Site - Home",
        "Home page of the basic OpenSpider fixture site.",
        "Basic Site",
        "Welcome to the basic fixture site used for export, CLI and agent tests.",
        canonical="{{BASE_URL}}/",
        nav_html=nav,
        extra=(
            '<img src="/assets/logo.png" alt="Basic Site logo" width="8" height="8">\n'
            f'<p><a href="{s.ext("/about.html")}">Partner site (external host)</a></p>\n'
        ),
    )
    # 2 about
    page(
        "/about.html",
        "About - Basic Site",
        "About the basic OpenSpider fixture site.",
        "About",
        "This is the about page.",
        canonical="{{BASE_URL}}/about.html",
        extra='<p><a href="/">Home</a> <a href="/products.html">Products</a> <a href="/contact.html">Contact</a></p>\n',
    )
    # 3 products (verlinkt Unterseite und Bild)
    page(
        "/products.html",
        "Products - Basic Site",
        "Products of the basic OpenSpider fixture site.",
        "Products",
        "This is the products page.",
        canonical="{{BASE_URL}}/products.html",
        extra='<p><a href="/products/widget.html">Widget</a> <a href="/">Home</a> <a href="/about.html">About</a></p>\n',
    )
    # 4 products/widget
    page(
        "/products/widget.html",
        "Widget - Basic Site",
        "The widget product page.",
        "Widget",
        "A widget with a picture.",
        extra='<img src="/assets/widget.png" alt="Widget" width="8" height="8">\n<p><a href="/products.html">Back to products</a></p>\n',
    )
    # 5 services
    page(
        "/services.html",
        "Services - Basic Site",
        "Services offered on the basic OpenSpider fixture site.",
        "Services",
        "We offer several services.",
        extra='<p><a href="/contact.html">Contact us</a> <a href="/faq.html">FAQ</a></p>\n',
    )
    # 6 contact (verlinkt den ausgeschlossenen Bereich /privat/)
    page(
        "/contact.html",
        "Contact - Basic Site",
        "Contact page of the basic OpenSpider fixture site.",
        "Contact",
        "Reach us by mail.",
        extra='<p><a href="/privat/intern.html">Internal area</a> <a href="/imprint.html">Imprint</a></p>\n',
    )
    # 7 faq
    page(
        "/faq.html",
        "FAQ - Basic Site",
        "Frequently asked questions.",
        "FAQ",
        "Questions and answers.",
        extra='<p><a href="/services.html">Services</a></p>\n',
    )
    # 8 imprint
    page(
        "/imprint.html",
        "Imprint - Basic Site",
        "Imprint of the basic OpenSpider fixture site.",
        "Imprint",
        "Legal notice.",
        extra='<p><a href="/privat/intern.html">Internal area</a> <a href="/orphan.html">Orphan</a></p>\n',
    )
    # 9 blog index
    page(
        "/blog/",
        "Blog - Basic Site",
        "Blog of the basic OpenSpider fixture site.",
        "Blog",
        "Latest articles.",
        extra='<p><a href="/blog/post-1.html">First post</a></p>\n',
    )
    # 10 blog post
    page(
        "/blog/post-1.html",
        "First post - Basic Site",
        "The first blog post.",
        "First post",
        "Hello world.",
        extra='<p><a href="/blog/">Back to the blog</a></p>\n',
    )
    # 11 privat/intern (durch exclude.osconfig ausgeschlossen)
    page(
        "/privat/intern.html",
        "Internal area - Basic Site",
        "Internal area, excluded by exclude.osconfig.",
        "Internal area",
        "Nothing to see here.",
        extra='<p><a href="/">Home</a></p>\n',
    )
    # 12 orphan (nur vom Impressum verlinkt, nicht in der Sitemap; so erscheinen alle 12 Seiten im Crawl)
    page(
        "/orphan.html",
        "Orphan - Basic Site",
        "A page that is linked only from the imprint.",
        "Orphan",
        "This page is linked only from the imprint and is not part of the sitemap.",
    )

    # Bilder, CSS, JS
    s.file("/assets/logo.png", png(color=(30, 90, 160)))
    s.file("/assets/widget.png", png(color=(200, 90, 30)))
    s.file("/assets/site.css", "body { font-family: sans-serif; }\nnav a { margin-right: 0.5em; }\n")
    s.file("/assets/site.js", "// Fixture-Skript ohne Verhalten.\nvar basicSite = true;\n")

    # robots.txt / sitemap.xml (siehe Modulkommentar)
    s.file("/robots.txt", "User-agent: *\nDisallow:\n\nSitemap: {{BASE_URL}}/sitemap.xml\n")
    s.file(
        "/sitemap.xml",
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
        "  <url><loc>{{BASE_URL}}/</loc></url>\n"
        "  <url><loc>{{BASE_URL}}/about.html</loc></url>\n"
        "  <url><loc>{{BASE_URL}}/products.html</loc></url>\n"
        "</urlset>\n",
    )

    # 301 und 404 (wie im Szenario seit P00b)
    s.route("/old-page", status=301, location="/about.html")
    s.route(
        "/missing.html",
        status=404,
        body="<!doctype html><html><head><title>Not Found</title></head><body><h1>404</h1></body></html>",
    )
    return s
