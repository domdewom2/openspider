#!/usr/bin/env python3
"""Erzeugt die ISO-Code-Tabellen fuer die hreflang-Validierung (TECH-072) aus den Unicode-CLDR-Validitaetsdaten.

Quelle:   https://github.com/unicode-org/cldr  (common/validity/{language,region,script}.xml)
Lizenz:   Unicode License v3 (SPDX: Unicode-3.0); Lizenztext: LICENSE-Unicode-3.0.txt in diesem Ordner
Aufruf:   python3 -I gen_iso.py [--tag release-48] [--out <Ordner>]

Das Skript laedt die Quelldateien per `curl` in ein frisches temporaeres Verzeichnis (heruntergeladene Daten
werden nie aus dem Skriptordner heraus gelesen) und schreibt in den Ausgabeordner (Standard: Ordner dieses
Skripts):

  iso639-1.txt          zweibuchstabige Sprachcodes (CLDR `language`, Status `regular`, Laenge 2)
  iso3166-1-alpha2.txt  Laendercodes (CLDR `region`, Status `regular`, Laenge 2)
  iso15924.txt          Schriftcodes (CLDR `script`, Status `regular`, Laenge 4)
  LICENSE-Unicode-3.0.txt

Sprachcodes: CLDR fuehrt einige gueltige ISO-639-1-Codes (`bh`, `tl`, `tw`) wegen bevorzugter anderer BCP-47-Codes
als `deprecated`; sie werden zusaetzlich aufgenommen (ergibt die 184 Codes von ISO 639-1). Die tatsaechlich
zurueckgezogenen Codes (`in`, `iw`, `ji`, `jw`, `mo`, `sh`) bleiben ausgeschlossen (TECH-072: ungueltig).
Die Bereichsschreibweise der Quelle (`aaa~i` = aaa bis aai, nur das letzte Zeichen variiert)
wird hier aufgeloest; die Ausgabe ist sortiert und enthaelt je Zeile einen Code.
"""

import argparse
import os
import re
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

DEFAULT_TAG = "release-48"
RAW = "https://raw.githubusercontent.com/unicode-org/cldr/{tag}/{path}"

# (Quelldatei, Typ, gewuenschte Codelaenge, Ausgabedatei, Anzeigename)
TABLES = [
    ("language", 2, "iso639-1.txt", "ISO 639-1 (zweibuchstabige Sprachcodes)"),
    ("region", 2, "iso3166-1-alpha2.txt", "ISO 3166-1 alpha-2 (Laendercodes)"),
    ("script", 4, "iso15924.txt", "ISO 15924 (Schriftcodes)"),
]


def curl(url, dest):
    subprocess.run(["curl", "-fsSL", "--retry", "3", "-o", dest, url], check=True)


def expand(token):
    """Loest ein Element der Validitaetsschreibweise auf: `abc` oder `aaa~i` (nur das letzte Zeichen variiert)."""
    if "~" not in token:
        return [token]
    start, end = token.split("~")
    if len(end) != 1:
        raise ValueError(f"unerwartete Bereichsform: {token}")
    first = start[-1]
    if ord(end) < ord(first):
        raise ValueError(f"absteigender Bereich: {token}")
    return [start[:-1] + chr(c) for c in range(ord(first), ord(end) + 1)]


# Von ISO 639-1 zurueckgezogene Codes (bleiben ungueltig, auch wenn CLDR sie als `deprecated` fuehrt).
WITHDRAWN_639_1 = {"in", "iw", "ji", "jw", "mo", "sh"}


def regular_codes(xml_path, kind):
    root = ET.parse(xml_path).getroot()
    codes = []
    for node in root.iter("id"):
        status = node.get("idStatus")
        wanted = status == "regular" or (kind == "language" and status == "deprecated")
        if node.get("type") == kind and wanted:
            for token in (node.text or "").split():
                codes.extend(expand(token))
    return codes


def header(tag, kind, title, count):
    return "\n".join(
        [
            f"# {title}",
            "# Erzeugt von gen_iso.py - nicht von Hand aendern.",
            f"# Quelle: https://github.com/unicode-org/cldr/blob/{tag}/common/validity/{kind}.xml",
            f"# CLDR-Version: {tag}; verwendet wird die Kategorie idStatus='regular' (Sprachen: Ausnahmen siehe gen_iso.py).",
            "# Copyright © 1991-2024 Unicode, Inc. (Angabe aus der Quelldatei).",
            "# Lizenz: Unicode License v3 (SPDX: Unicode-3.0), siehe LICENSE-Unicode-3.0.txt.",
            f"# Eintraege: {count}; eine Zeile je Code, Zeilen mit # sind Kommentare.",
            "",
        ]
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tag", default=DEFAULT_TAG)
    ap.add_argument("--out", default=os.path.dirname(os.path.abspath(__file__)))
    args = ap.parse_args()

    with tempfile.TemporaryDirectory(prefix="cldr-validity-") as tmp:
        for kind, length, name, title in TABLES:
            src = os.path.join(tmp, f"{kind}.xml")
            curl(RAW.format(tag=args.tag, path=f"common/validity/{kind}.xml"), src)
            codes = sorted({c for c in regular_codes(src, kind) if len(c) == length})
            if kind == "language":
                codes = [c for c in codes if c not in WITHDRAWN_639_1]
            pattern = {2: r"^[A-Za-z]{2}$", 4: r"^[A-Za-z]{4}$"}[length]
            bad = [c for c in codes if not re.match(pattern, c)]
            if not codes or bad:
                sys.exit(f"{kind}: unerwartete Daten ({len(codes)} Codes, ungueltig: {len(bad)})")
            with open(os.path.join(args.out, name), "w", encoding="utf-8", newline="\n") as f:
                f.write(header(args.tag, kind, title, len(codes)))
                f.write("\n".join(codes) + "\n")
            print(f"{name}: {len(codes)} Codes")
        curl(
            RAW.format(tag=args.tag, path="LICENSE"),
            os.path.join(args.out, "LICENSE-Unicode-3.0.txt"),
        )


if __name__ == "__main__":
    main()
