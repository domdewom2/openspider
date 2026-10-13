"""parity-response (Port 8904): Statuscodes, Weiterleitungen (HTTP, Meta Refresh, Refresh-Header),
Ketten, Schleifen, robots-gesperrt, externe Ziele, Inlink-Bulk."""

from lib import *

PORT = 8904


def build() -> Site:
    s = Site("parity-response", PORT)
    s.file("/robots.txt", "User-agent: *\nDisallow: /blocked/\n")

    def body(title: str) -> str:
        return simple_html(title)

    items = []

    def add(path, text):
        items.append((path, text))

    # 2xx
    std(s, "/ok.html", "response ok", parent="/", p_words=120)
    add("/ok.html", "ok (200)")
    s.route("/s201", status=201, body=body("Created 201"))
    s.route("/s204", status=204)
    add("/s201", "201")
    add("/s204", "204")

    # Weiterleitungen
    for code in (301, 302, 303, 307, 308):
        s.route(f"/r{code}", status=code, location="/ok.html")
        add(f"/r{code}", f"redirect {code}")
    s.route("/r304", status=304)
    s.route("/r301-noloc", status=301)
    add("/r304", "304 without Location")
    add("/r301-noloc", "301 without Location")

    # Fehler
    for code in (400, 403, 404, 410, 429, 500, 503):
        s.route(f"/e{code}", status=code, body=body(f"Error {code}"))
        add(f"/e{code}", f"error {code}")
    s.route("/s999", status=999, body=body("Status 999"))
    add("/s999", "status 999")

    # Meta Refresh / Refresh-Header
    s.file("/meta-refresh.html", simple_html("Meta refresh immediately", extra="").replace(
        "<title>", '<meta http-equiv="refresh" content="0; url=/ok.html">\n<title>', 1))
    s.file("/meta-refresh-5.html", simple_html("Meta refresh delayed").replace(
        "<title>", "<meta http-equiv=\"refresh\" content=\"5;URL='/ok.html'\">\n<title>", 1))
    s.file("/meta-refresh-self.html", simple_html("Meta refresh without target").replace(
        "<title>", '<meta http-equiv="refresh" content="30">\n<title>', 1))
    s.file("/mixed-chain.html", simple_html("Meta refresh into a redirect").replace(
        "<title>", '<meta http-equiv="refresh" content="0; url=/r301">\n<title>', 1))
    s.file("/http-refresh.html", simple_html("HTTP Refresh header"))
    s.route("/http-refresh.html", headers={"Refresh": "0; url=/ok.html"})
    s.file("/http-refresh-nourl.html", simple_html("HTTP Refresh header without target"))
    s.route("/http-refresh-nourl.html", headers={"Refresh": "10"})
    for p, t in (("/meta-refresh.html", "meta refresh 0"), ("/meta-refresh-5.html", "meta refresh 5 with quotes"),
                 ("/meta-refresh-self.html", "meta refresh without target"), ("/mixed-chain.html", "meta refresh then 301"),
                 ("/http-refresh.html", "Refresh header"), ("/http-refresh-nourl.html", "Refresh header without target")):
        add(p, t)

    # Ketten und Schleifen
    s.route("/chain1", status=301, location="/chain2")
    s.route("/chain2", status=301, location="/ok.html")
    add("/chain1", "chain, 2 hops")
    for i in range(1, 6):
        target = f"/long{i + 1}" if i < 5 else "/ok.html"
        s.route(f"/long{i}", status=301, location=target)
    add("/long1", "chain, 5 hops")
    s.route("/loop1", status=302, location="/loop2")
    s.route("/loop2", status=302, location="/loop1")
    add("/loop1", "loop")

    # Weiterleitungen auf Fehlerziele (Report "Redirects to Errors", OUT-084): 4xx, robots.txt-gesperrt,
    # 5xx nach zwei Spruengen, keine Antwort (externer Host, geschlossener Port), Meta Refresh auf 404.
    s.route("/r-to-404", status=301, location="/e404")
    s.route("/r-to-blocked", status=302, location="/blocked/a.html")
    s.route("/r-chain-500", status=301, location="/r-chain-500b")
    s.route("/r-chain-500b", status=301, location="/e500")
    s.route("/r-to-dead", status=301, location="http://localhost:1/")
    s.file("/r-meta-to-404.html", simple_html("Meta refresh into a 404").replace(
        "<title>", '<meta http-equiv="refresh" content="0; url=/e404">\n<title>', 1))
    for p, t in (("/r-to-404", "redirect to 404"), ("/r-to-blocked", "redirect to robots.txt blocked page"),
                 ("/r-chain-500", "redirect chain to 500"), ("/r-to-dead", "redirect to unreachable host"),
                 ("/r-meta-to-404.html", "meta refresh to 404")):
        add(p, t)

    # robots.txt-gesperrt
    std(s, "/blocked/a.html", "response blocked a", parent="/", p_words=60)
    std(s, "/blocked/b.html", "response blocked b", parent="/", p_words=60)
    add("/blocked/a.html", "blocked by robots.txt")
    add("/blocked/b.html", "blocked by robots.txt (second)")

    # externe Ziele (localhost = externer Host; Port wie die Site selbst bzw. geschlossen)
    ext = [("/ok.html", "external 200"), ("/e404", "external 404"), ("/e500", "external 500"), ("/r301", "external redirect")]
    for p, t in ext:
        add(s.ext(p), t)
    dead = "http://localhost:1/"
    add(dead, "external host, closed port")

    # Seiten, die gemeinsam auf Fehlerziele verlinken (Inlinks-Bulk)
    for n in range(1, 5):
        std(
            s,
            f"/pages/p{n}.html",
            f"response page {n}",
            parent="/",
            p_words=100,
            body_extra=link_list([("/e404", "dead link"), ("/r301", "redirect link"), ("/ok.html", "ok link")]),
        )
        add(f"/pages/p{n}.html", f"page {n} (links to errors)")

    std(s, "/index.html", "parity response home", nav=False, body_extra=link_list(items), p_words=60)

    # --- Coverage -------------------------------------------------------------------------
    codes_ok = ["/ok.html", "/s201", "/s204"]
    r3xx = ["/r301", "/r302", "/r303", "/r307", "/r308", "/r304", "/r301-noloc",
            "/r-to-404", "/r-to-blocked", "/r-chain-500", "/r-chain-500b", "/r-to-dead"]
    c4 = ["/e400", "/e403", "/e404", "/e410", "/e429"]
    c5 = ["/e500", "/e503"]
    meta = ["/meta-refresh.html", "/meta-refresh-5.html", "/r-meta-to-404.html"]
    chain = ["/chain1", "/long1", "/mixed-chain.html", "/r-chain-500"]
    loops = ["/loop1", "/loop2"]
    blocked = ["/blocked/a.html", "/blocked/b.html"]
    other = ["/s999"]

    def both(tab_prefix, name, hit_int, miss_int, hit_ext=None, miss_ext=None):
        # Sicht ohne Praefix = intern + extern; "Internal ..." / "External ..." disjunkte Teilmengen
        hit_ext = hit_ext or []
        miss_ext = miss_ext or []
        s.case("Response Codes", name, hit=hit_int + hit_ext, miss=miss_int + miss_ext)
        s.case("Response Codes", "Internal " + name, hit=hit_int, miss=miss_int + hit_ext)
        if hit_ext:
            s.case("Response Codes", "External " + name, hit=hit_ext, miss=miss_int + miss_ext)

    s.case("Response Codes", "All", hit=["/ok.html", "ext:/ok.html"])
    s.exempt_filter("Response Codes", "All", "Grundmenge aller URLs: es gibt keinen Nicht-Treffer (Internal All/External All grenzen ab)")
    s.case("Response Codes", "Internal All", hit=["/ok.html", "/e404"], miss=["ext:/ok.html"])
    s.case("Response Codes", "External All", hit=["ext:/ok.html", "ext:/e404"], miss=["/ok.html"])
    both("", "Blocked by Robots.txt", blocked, ["/ok.html", "/e404"])
    s.exempt_filter("Response Codes", "External Blocked by Robots.txt",
                    "Verhalten der Referenz fuer robots.txt externer Hosts nicht festgelegt (ONPAGE-052, A-17)")
    s.case("Response Codes", "No Response", hit=["dead:" + dead], miss=["/e404", "/ok.html"])
    s.case("Response Codes", "External No Response", hit=["dead:" + dead], miss=["/e404", "ext:/e500"])
    s.exempt_filter("Response Codes", "Internal No Response",
                    "braucht einen internen Abbruch (hang/abort_after_headers) aus M1-P09; mit der aktuellen Testsite nicht darstellbar")
    both("", "Success (2XX)", codes_ok, ["/r301", "/e404"], ["ext:/ok.html"], [])
    both("", "Redirection (3XX)", r3xx, ["/ok.html", "/e404"], ["ext:/r301"], [])
    both("", "Redirection (Meta Refresh)", meta, ["/meta-refresh-self.html", "/ok.html"])
    s.exempt_filter("Response Codes", "External Redirection (Meta Refresh)",
                    "externe Seiten werden nur auf den Status geprueft, nicht geparst (ONPAGE-025)")
    both("", "Redirect Chain", chain, ["/r301", "/chain2", "/loop1", "/r-to-404"])
    s.exempt_filter("Response Codes", "External Redirect Chain", "Ketten gelten nur fuer interne URLs (ONPAGE-060)")
    both("", "Redirect Loop", loops, ["/chain1", "/r301"])
    s.exempt_filter("Response Codes", "External Redirect Loop", "Schleifen gelten nur fuer interne URLs (ONPAGE-061)")
    both("", "Client Error (4XX)", c4, ["/ok.html", "/e500"], ["ext:/e404"], [])
    both("", "Server Error (5XX)", c5, ["/e404", "/ok.html"], ["ext:/e500"], [])
    both("", "Other", other, ["/ok.html", "/e404"])
    s.exempt_filter("Response Codes", "External Other",
                    "kein externer Status ausserhalb 100-599 vorgesehen (ONPAGE-066 ist P2)")

    # Report "Redirects to Errors" (OUT-084): Startadresse -> Endziel nach Filterzuordnung. Treffer = Endziel ist
    # "Blocked by Robots.txt", "No Response", "Client Error (4XX)" oder "Server Error (5XX)"; Nicht-Treffer = 200 oder Schleife.
    s.report(
        "Redirects to Errors",
        hit=[
            "/r-to-404 -> Client Error (4XX)",
            "/r-to-blocked -> Blocked by Robots.txt",
            "/r-chain-500 -> Server Error (5XX)",
            "/r-to-dead -> No Response",
            "/r-meta-to-404.html -> Client Error (4XX)",
        ],
        miss=["/r301", "/chain1", "/mixed-chain.html", "/loop1", "/meta-refresh.html"],
    )
    return s
