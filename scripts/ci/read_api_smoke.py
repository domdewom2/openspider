#!/usr/bin/env python3
"""Beispielleser der stabilen Leseschnittstelle einer OpenSpider-Crawl-Datei (OUT-233, OUT-234).

Der Leser benutzt nur die Python-Standardbibliothek (sqlite3, hashlib) und keinen OpenSpider-Code: Er öffnet die Datei
mit `mode=ro` (bei schreibgeschütztem Verzeichnis mit `immutable=1`), prüft `format` und `read_api_version` im Manifest,
liest alle `q_`-Sichten, filtert und sortiert auf jeder Spalte und gleicht `config_sha256` mit einer eigenen SHA-256-
Berechnung über `q_config.osconfig` ab.

Aufruf:  read_api_smoke.py <datei.ospider> [--immutable] [--repeat N] [--json]

Ausgang:  0 = alles gelesen, 1 = Prüfung fehlgeschlagen, 2 = Aufruf falsch, 3 = unbekannte Version der Leseschnittstelle.
Mit `--repeat N` liest der Leser die Datei N-mal hintereinander (für den Lauf neben einem Crawl: kein Sperrfehler, der
Leser blockiert den Schreiber nicht).
"""

import argparse
import hashlib
import json
import sqlite3
import sys
import time
from pathlib import Path
from urllib.parse import quote

SUPPORTED_READ_API_VERSION = "1"
VIEWS = ["q_manifest", "q_config", "q_urls", "q_links", "q_sitemap_urls", "q_columns"]
MANIFEST_KEYS = [
    "format", "manifest_version", "read_api_version", "schema_version", "openspider_version", "last_writer_version",
    "writer_kind", "crawl_id", "name", "project", "mode", "start_url", "list_source", "started_at", "ended_at", "state",
    "end_reason", "completeness", "user_agent", "config_sha256", "urls_total", "urls_crawled", "analysis_state",
]


class UnknownReadApiVersion(Exception):
    """Die Datei nennt eine Hauptversion der Leseschnittstelle, die dieser Leser nicht kennt."""


class CheckFailed(Exception):
    """Eine Prüfung ist fehlgeschlagen."""


def open_readonly(path: Path, immutable: bool) -> sqlite3.Connection:
    """Öffnet die Datei nur lesend, ohne OpenSpider-Kollationen oder -Funktionen."""
    uri = "file:" + quote(str(path.resolve())) + ("?mode=ro&immutable=1" if immutable else "?mode=ro")
    conn = sqlite3.connect(uri, uri=True, timeout=5.0)
    conn.row_factory = sqlite3.Row
    return conn


def read_manifest(conn: sqlite3.Connection) -> dict:
    """Liest das Manifest und prüft Format und Hauptversion der Leseschnittstelle."""
    try:
        rows = conn.execute("SELECT key, value FROM q_manifest").fetchall()
    except sqlite3.Error as e:
        raise CheckFailed(f"q_manifest nicht lesbar (keine OpenSpider-Crawl-Datei mit Leseschnittstelle?): {e}")
    manifest = {r["key"]: r["value"] for r in rows}
    if manifest.get("format") != "ospider":
        raise CheckFailed(f"keine OpenSpider-Crawl-Datei (format = {manifest.get('format')!r})")
    version = manifest.get("read_api_version")
    if version != SUPPORTED_READ_API_VERSION:
        raise UnknownReadApiVersion(f"unbekannte Version der Leseschnittstelle: {version}")
    missing = [k for k in MANIFEST_KEYS if k not in manifest]
    if missing:
        raise CheckFailed(f"Manifest ohne Schlüssel: {missing}")
    return manifest


def columns_of(conn: sqlite3.Connection, view: str) -> list:
    return [r["name"] for r in conn.execute(f"PRAGMA table_info({view})").fetchall()]


def check_views(conn: sqlite3.Connection) -> dict:
    """Liest jede Sicht, filtert und sortiert auf jeder Spalte; prüft die Selbstbeschreibung `q_columns`."""
    described = {(r["view"], r["column"]): (r["type"], r["description"])
                 for r in conn.execute('SELECT "view", "column", type, description FROM q_columns').fetchall()}
    counts = {}
    for view in VIEWS:
        cols = columns_of(conn, view)
        if not cols:
            raise CheckFailed(f"Sicht {view} fehlt")
        counts[view] = conn.execute(f"SELECT COUNT(*) FROM {view}").fetchone()[0]
        for col in cols:
            info = described.get((view, col))
            if info is None:
                raise CheckFailed(f"q_columns beschreibt {view}.{col} nicht")
            sql_type, description = info
            if sql_type not in ("INTEGER", "REAL", "TEXT") or not (description or "").strip():
                raise CheckFailed(f"q_columns: {view}.{col} ohne Typ oder Beschreibung")
            q = f'"{col}"'
            # Sortieren (auf- und absteigend) und Filtern: reines Standard-SQLite, keine eigene Kollation nötig.
            conn.execute(f"SELECT {q} FROM {view} ORDER BY {q} LIMIT 200").fetchall()
            conn.execute(f"SELECT {q} FROM {view} ORDER BY {q} DESC LIMIT 200").fetchall()
            conn.execute(f"SELECT COUNT(*) FROM {view} WHERE {q} IS NOT NULL").fetchone()
            if sql_type == "TEXT":
                conn.execute(f"SELECT COUNT(*) FROM {view} WHERE {q} LIKE ?", ("%a%",)).fetchone()
    for (view, col) in described:
        if view in VIEWS and col not in columns_of(conn, view):
            raise CheckFailed(f"q_columns beschreibt eine Spalte, die es nicht gibt: {view}.{col}")
    return counts


def check_config_hash(conn: sqlite3.Connection, manifest: dict) -> None:
    """`config_sha256` ist der SHA-256 (hex) der eingebetteten .osconfig – mit eigener Berechnung geprüft."""
    row = conn.execute("SELECT osconfig FROM q_config").fetchone()
    if row is None:
        raise CheckFailed("q_config ist leer")
    own = hashlib.sha256(row["osconfig"].encode("utf-8")).hexdigest()
    if own != manifest["config_sha256"]:
        raise CheckFailed(f"config_sha256 stimmt nicht: Datei {manifest['config_sha256']}, berechnet {own}")


def check_no_secrets(conn: sqlite3.Connection) -> None:
    """Keine Zugangsdaten in den Sichtausgaben (Volltextsuche wie OUT-230)."""
    needles = ["refresh_token", "access_token", "client_secret", "bearer ", "password =", "api_key =", "-----begin"]
    for view in VIEWS:
        for row in conn.execute(f"SELECT * FROM {view}"):
            text = " ".join(str(v) for v in tuple(row) if v is not None).lower()
            for n in needles:
                if n in text:
                    raise CheckFailed(f"{view}: enthält {n!r}")


def read_once(path: Path, immutable: bool) -> dict:
    conn = open_readonly(path, immutable)
    try:
        manifest = read_manifest(conn)
        counts = check_views(conn)
        check_config_hash(conn, manifest)
        check_no_secrets(conn)
        return {"manifest": manifest, "rows": counts}
    finally:
        conn.close()


def main(argv: list) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("file", type=Path)
    ap.add_argument("--immutable", action="store_true", help="mit immutable=1 öffnen (schreibgeschütztes Verzeichnis)")
    ap.add_argument("--repeat", type=int, default=1, help="so oft hintereinander lesen (Lauf neben einem Crawl)")
    ap.add_argument("--pause-ms", type=int, default=50, help="Pause zwischen zwei Durchläufen")
    ap.add_argument("--json", action="store_true", help="Ergebnis als JSON ausgeben")
    try:
        args = ap.parse_args(argv)
    except SystemExit:
        return 2
    if not args.file.is_file():
        print(f"Datei nicht gefunden: {args.file}", file=sys.stderr)
        return 2
    result = None
    try:
        for i in range(max(1, args.repeat)):
            result = read_once(args.file, args.immutable)
            if i + 1 < args.repeat:
                time.sleep(args.pause_ms / 1000.0)
    except UnknownReadApiVersion as e:
        print(str(e), file=sys.stderr)
        return 3
    except (CheckFailed, sqlite3.Error) as e:
        print(f"Prüfung fehlgeschlagen: {e}", file=sys.stderr)
        return 1
    m = result["manifest"]
    if args.json:
        print(json.dumps(result, sort_keys=True))
    else:
        rows = ", ".join(f"{v}={n}" for v, n in result["rows"].items())
        print(f"ok: {args.file.name} (schema {m['schema_version']}, read_api {m['read_api_version']}, "
              f"{m['completeness']}, urls {m['urls_total']}); {rows}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
