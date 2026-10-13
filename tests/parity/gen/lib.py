"""Gemeinsame Bausteine des Fixture-Generators (M1-P10).

Alles hier ist deterministisch: gleiche Eingabe, gleiche Bytes. Die Generatoren schreiben nur
nach `tests/fixtures/<site>/` und `tests/parity/coverage/<site>.toml`.
"""

from __future__ import annotations

import gzip
import html
import json
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
FIXTURES = ROOT / "tests" / "fixtures"
COVERAGE = ROOT / "tests" / "parity" / "coverage"
GENERATED_LIST = ROOT / "tests" / "parity" / "gen" / "generated.txt"

DEF = object()  # "Standardwert verwenden" (None bedeutet: Element weglassen)

VOCAB = (
    "alpha beta gamma delta kappa lambda sigma omega river stone forest meadow harbor lantern "
    "bridge garden window market valley summit anchor copper silver amber cedar maple willow "
    "orchard pebble ribbon thunder whisper compass journal letter morning evening season harvest "
    "kitchen library station engine signal pattern network archive ledger quartz marble velvet "
    "cotton linen granite timber meadow canyon glacier island lagoon prairie tundra savanna "
    "reef delta fjord plateau ridge basin dune cliff cove brook creek spring pond lake ocean "
    "breeze storm cloud frost dew mist haze rain snow sleet hail sun moon star comet planet "
    "orbit galaxy nebula cosmos horizon zenith meridian equator tropic polar arctic coastal "
    "inland upland lowland highland midland borderland homeland wasteland woodland grassland"
).split()


def esc(text: str) -> str:
    return html.escape(text, quote=True)


def _h(seed: str, i: int) -> int:
    return zlib.crc32(f"{seed}|{i}".encode("utf-8")) & 0xFFFFFFFF


def lorem(seed: str, n: int) -> str:
    """n deterministische Wörter (nur a-z), getrennt durch ein Leerzeichen."""
    return " ".join(VOCAB[_h(seed, i) % len(VOCAB)] for i in range(n))


def count_words(text: str) -> int:
    return len(text.split())


def fit_text(seed: str, lo: int, hi: int, label: str = "") -> str:
    """Lesbarer Text mit Länge in [lo, hi], beginnt mit `label` (falls gegeben)."""
    text = label.strip()
    i = 0
    while len(text) < lo:
        text = (text + " " + VOCAB[_h(seed, i) % len(VOCAB)]).strip()
        i += 1
    if len(text) > hi:
        text = text[:hi].rstrip()
    assert lo <= len(text) <= hi, (text, len(text))
    return text


def exact_text(seed: str, n: int, label: str = "") -> str:
    """Text mit genau n Zeichen (kein führendes/abschließendes Leerzeichen)."""
    text = label.strip()
    i = 0
    while len(text) < n + 1:
        text = (text + " " + VOCAB[_h(seed, i) % len(VOCAB)]).strip()
        i += 1
    text = text[:n]
    if text.endswith(" "):
        text = text[:-1] + "x"
    assert len(text) == n and text == text.strip(), (text, len(text), n)
    return text


# ---------------------------------------------------------------------------------------------
# Binärformate


def _chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)


def png(color=(40, 120, 200), size=(8, 8), exact_size: int | None = None) -> bytes:
    """Gültiges PNG; mit `exact_size` wird per tEXt-Chunk auf genau diese Bytezahl aufgefüllt."""
    w, h = size
    raw = b"".join(b"\x00" + bytes(color) * w for _ in range(h))
    head = b"\x89PNG\r\n\x1a\n" + _chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
    idat = _chunk(b"IDAT", zlib.compress(raw, 9))
    iend = _chunk(b"IEND", b"")
    data = head + idat
    if exact_size is not None:
        need = exact_size - len(data) - len(iend) - 12  # 12 = Länge + Typ + CRC des tEXt-Chunks
        assert need >= 8, (exact_size, len(data))
        data += _chunk(b"tEXt", b"Comment\x00" + b"x" * (need - 8))
    data += iend
    if exact_size is not None:
        assert len(data) == exact_size, (len(data), exact_size)
    return data


def svg(label: str = "fixture") -> bytes:
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">'
        f'<rect width="16" height="16" fill="#2878c8"/><!-- {label} --></svg>\n'
    ).encode("utf-8")


def pdf(text: str = "OpenSpider fixture") -> bytes:
    """Minimales, gültiges Ein-Seiten-PDF."""
    objs = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R "
        b"/Resources << /Font << /F1 5 0 R >> >> >>",
    ]
    stream = f"BT /F1 12 Tf 10 50 Td ({text}) Tj ET".encode("ascii")
    objs.append(b"<< /Length " + str(len(stream)).encode() + b" >>\nstream\n" + stream + b"\nendstream")
    objs.append(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
    out = b"%PDF-1.4\n"
    offsets = []
    for i, body in enumerate(objs, start=1):
        offsets.append(len(out))
        out += f"{i} 0 obj\n".encode() + body + b"\nendobj\n"
    xref = len(out)
    out += f"xref\n0 {len(objs) + 1}\n".encode() + b"0000000000 65535 f \n"
    for off in offsets:
        out += f"{off:010d} 00000 n \n".encode()
    out += f"trailer\n<< /Size {len(objs) + 1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    return out


def gz(data: bytes) -> bytes:
    return gzip.compress(data, mtime=0)


# ---------------------------------------------------------------------------------------------
# TOML-Ausgabe


def toml_str(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def toml_list(values) -> str:
    return "[" + ", ".join(toml_str(v) for v in values) + "]"


# ---------------------------------------------------------------------------------------------
# Site


class Site:
    def __init__(self, name: str, port: int):
        self.name = name
        self.port = port
        self.files: dict[str, bytes] = {}
        self.routes: list[dict] = []
        self.cases: list[dict] = []
        self.exempt: list[dict] = []
        self.reports: list[dict] = []

    # --- Adressen -------------------------------------------------------------------------
    def ext(self, path: str) -> str:
        """Absoluter Link auf den externen Host `localhost` (fester Manifest-Port)."""
        return f"http://localhost:{self.port}{path}"

    def internal_abs(self, path: str) -> str:
        return f"http://127.0.0.1:{self.port}{path}"

    # --- Inhalt ---------------------------------------------------------------------------
    def file(self, url_path: str, content) -> None:
        if isinstance(content, str):
            content = content.encode("utf-8")
        rel = url_path.lstrip("/")
        if rel == "" or rel.endswith("/"):
            rel += "index.html"
        assert rel not in self.files, f"doppelt: {rel}"
        self.files[rel] = content

    def route(self, path: str, **kw) -> None:
        r = {"path": path}
        r.update({k: v for k, v in kw.items() if v is not None})
        self.routes.append(r)

    def case(self, tab: str, flt: str, hit=(), miss=()) -> None:
        self.cases.append({"tab": tab, "filter": flt, "hit": list(hit), "miss": list(miss)})

    def report(self, name: str, hit=(), miss=()) -> None:
        """Absichtserklaerung zu einem Report (kein Filter): hit-Eintraege "<Adresse> -> <Endziel-Filter>"."""
        self.reports.append({"name": name, "hit": list(hit), "miss": list(miss)})

    def exempt_filter(self, tab: str, flt: str, reason: str) -> None:
        self.exempt.append({"tab": tab, "filter": flt, "reason": reason})

    # --- Ausgabe --------------------------------------------------------------------------
    def scenario_toml(self) -> str:
        lines = [
            f"# Szenario \"{self.name}\" (Port {self.port}) - erzeugt von tests/parity/gen/generate.py,",
            "# nicht von Hand bearbeiten (Aenderungen im Generator vornehmen und neu erzeugen).",
            "",
        ]
        for r in sorted(self.routes, key=lambda x: x["path"]):
            lines.append("[[route]]")
            lines.append(f"path = {toml_str(r['path'])}")
            for key in ("status", "location", "content_type", "delay_ms"):
                if key in r:
                    v = r[key]
                    lines.append(f"{key} = {v}" if isinstance(v, int) else f"{key} = {toml_str(v)}")
            if "body" in r:
                lines.append(f"body = {toml_str(r['body'])}")
            if r.get("headers"):
                lines.append("[route.headers]")
                for k, v in r["headers"].items():
                    lines.append(f"{toml_str(k)} = {toml_str(v)}")
            lines.append("")
        return "\n".join(lines) + "\n"

    def coverage_toml(self) -> str:
        lines = [
            f"# Absichtserklaerung der Fixture \"{self.name}\" je Tab/Filter (erzeugt von tests/parity/gen/generate.py).",
            "# hit  = URLs, die nach Spec im Filter stehen muessen; miss = URLs, die nicht darin stehen duerfen.",
            "# Adressen: \"/pfad\" intern, \"ext:/pfad\" externer Host localhost, \"dead:<url>\" nicht erreichbar.",
            "# [[report]] = Absichtserklaerung zu einem Report ohne Filter (z. B. Redirects to Errors): hit/miss",
            "# als \"<Adresse> -> <Endziel>\" (Treffer) bzw. Adresse (Nicht-Treffer).",
            "# [[report]] = Absichtserklaerung zu einem Report ohne Filter (z. B. Redirects to Errors).",
            "# Wird von tests/parity/fixtures_check.rs gegen die ausgelieferte Site geprueft.",
            f"site = {toml_str(self.name)}",
            "",
        ]
        for c in self.cases:
            lines.append("[[case]]")
            lines.append(f"tab = {toml_str(c['tab'])}")
            lines.append(f"filter = {toml_str(c['filter'])}")
            lines.append(f"hit = {toml_list(c['hit'])}")
            lines.append(f"miss = {toml_list(c['miss'])}")
            lines.append("")
        for rp in self.reports:
            lines.append("[[report]]")
            lines.append(f"name = {toml_str(rp['name'])}")
            lines.append(f"hit = {toml_list(rp['hit'])}")
            lines.append(f"miss = {toml_list(rp['miss'])}")
            lines.append("")
        for x in self.exempt:
            lines.append("[[exempt]]")
            lines.append(f"tab = {toml_str(x['tab'])}")
            lines.append(f"filter = {toml_str(x['filter'])}")
            lines.append(f"reason = {toml_str(x['reason'])}")
            lines.append("")
        return "\n".join(lines) + "\n"


# ---------------------------------------------------------------------------------------------
# Seitenbau


def doc(head: str, body: str, lang: str = "en") -> str:
    return (
        "<!doctype html>\n"
        f'<html lang="{lang}">\n<head>\n<meta charset="utf-8">\n{head}</head>\n<body>\n{body}</body>\n</html>\n'
    )


def _many(value, default):
    """DEF -> [default]; None -> []; str -> [str]; Liste -> Liste."""
    if value is DEF:
        return [default]
    if value is None:
        return []
    if isinstance(value, str):
        return [value]
    return list(value)


def std(
    site: Site,
    path: str,
    slug: str,
    *,
    title=DEF,
    desc=DEF,
    h1=DEF,
    h2=DEF,
    robots=None,
    canonical=None,
    hreflang=None,
    head_extra: str = "",
    body_extra: str = "",
    p_words: int | None = 230,
    nav: bool = True,
    parent: str = "/",
    footer: str | None = "Fixture footer text for parity checks.",
    store: bool = True,
) -> str:
    """Standardseite. `title`/`desc`/`h1`/`h2`: DEF = eindeutiger Standardwert, None = weglassen,
    str/Liste = genau diese Werte (Leerstring erzeugt ein leeres Element)."""
    label = slug.replace("-", " ").replace("/", " ").strip()
    titles = _many(title, fit_text("t" + path, 34, 56, label))
    descs = _many(desc, fit_text("d" + path, 90, 140, "Description of " + label))
    h1s = _many(h1, fit_text("h" + path, 20, 60, label))
    h2s = _many(h2, fit_text("s" + path, 20, 60, "Section " + label))

    head = ""
    for t in titles:
        head += f"<title>{esc(t)}</title>\n"
    for d in descs:
        head += f'<meta name="description" content="{esc(d)}">\n'
    for r in robots or []:
        head += f'<meta name="robots" content="{esc(r)}">\n'
    for c in canonical or []:
        head += f'<link rel="canonical" href="{esc(c)}">\n'
    for code, href in hreflang or []:
        head += f'<link rel="alternate" hreflang="{esc(code)}" href="{esc(href)}">\n'
    head += head_extra

    body = ""
    if nav:
        body += f'<nav><a href="/">Home</a> <a href="{esc(parent)}">Section</a></nav>\n'
    body += "<main>\n"
    for x in h1s:
        body += f"<h1>{esc(x)}</h1>\n"
    for x in h2s:
        body += f"<h2>{esc(x)}</h2>\n"
    if p_words:
        body += f"<p>{lorem('p' + path, p_words)}</p>\n"
    body += body_extra
    body += "</main>\n"
    if footer:
        body += f"<footer><p>{esc(footer)}</p></footer>\n"
    text = doc(head, body)
    if store:
        site.file(path, text)
    return text


def link_list(items) -> str:
    """items: (href, text)."""
    out = "<ul>\n"
    for href, text in items:
        out += f'<li><a href="{esc(href)}">{esc(text)}</a></li>\n'
    return out + "</ul>\n"


def hub(site: Site, path: str, slug: str, items, parent: str = "/", extra: str = "") -> None:
    std(
        site,
        path,
        slug,
        body_extra=link_list(items) + extra,
        p_words=60,
        parent=parent,
    )


def simple_html(title: str, h1: str | None = None, extra: str = "") -> str:
    h = h1 if h1 is not None else title
    return (
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">"
        f"<title>{esc(title)}</title></head><body><h1>{esc(h)}</h1>{extra}</body></html>\n"
    )


# ---------------------------------------------------------------------------------------------
# Schreiben


def write_site(site: Site) -> list[str]:
    """Schreibt Dateien, scenario.toml und die Coverage-Datei; gibt die geschriebenen Pfade zurück
    (relativ zum Repo-Wurzelverzeichnis)."""
    base = FIXTURES / site.name
    written: list[str] = []
    for rel, data in sorted(site.files.items()):
        target = base / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        written.append(target.relative_to(ROOT).as_posix())
    scenario = base / "scenario.toml"
    if site.routes or not site.files:
        scenario.write_text(site.scenario_toml(), encoding="utf-8", newline="\n")
        written.append(scenario.relative_to(ROOT).as_posix())
    if site.cases or site.exempt or site.reports:
        COVERAGE.mkdir(parents=True, exist_ok=True)
        cov = COVERAGE / f"{site.name}.toml"
        cov.write_text(site.coverage_toml(), encoding="utf-8", newline="\n")
        written.append(cov.relative_to(ROOT).as_posix())
    return written
