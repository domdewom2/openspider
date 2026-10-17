#!/usr/bin/env python3
"""Schattenvergleich E1 (M2-V02): OpenSpider-Exporte gegen die Befunde des Fremdcrawlers (nur lesend).

Python 3.12, nur Standardbibliothek. Unterbefehle:

  exports          Liste der zu exportierenden Tab:Filter-Kennungen aus rule-map.toml (für run.sh)
  fetch-autonomo   liest die Befunde des zeitlich nächsten abgeschlossenen Autonomo-Crawls einer Site
                   (ausschließlich GET bzw. SELECT) und schreibt sie normalisiert als JSON
  discover         zeigt nur die Struktur (Schlüssel, Typen, Anzahlen) einer Autonomo-Antwort, nie Werte
  compare          vergleicht die CSV-Exporte eines OpenSpider-Laufs mit der normalisierten Autonomo-Datei
  metrics          Rate (Mittel, Höchstwert, 10-s-Fenster) und Spitzen-RSS aus Fortschrittsdatei und run.json

Sicherheitsregeln (docs/plan/M2.md 3.13):
  * Der Bearer-Token wird nur zur Laufzeit aus der bestehenden Autonomo-Konfiguration gelesen, nie
    kopiert, nie geloggt, nie in Argumenten oder Fehlermeldungen genannt; Ziel nur Loopback.
  * Kein POST/PUT/PATCH/DELETE; die Postgres-Lesesitzung läuft mit default_transaction_read_only=on und
    akzeptiert ausschließlich eine einzelne SELECT-Anweisung.
  * Geschrieben wird nur in das mit --out angegebene Verzeichnis (unter /opt/openspider-shadow/var; mit --base
    erzwungen). Site-Kennungen werden vor jeder Pfadbildung auf Hostname-Zeichen geprüft.
  * Aus Autonomo-Beständen werden nur Kennungen (rule_id) und URLs übernommen, keine Texte.

URL-Normalisierung (identisch für beide Seiten, siehe normalize_url):
  Schema und Host kleingeschrieben, Fragment entfernt, Standardport (80 bei http, 443 bei https) entfernt,
  Benutzerinfo entfernt, Prozent-Kodierung vereinheitlicht (Großbuchstaben, unreservierte Zeichen
  dekodiert). Slash-Regel: ein leerer Pfad wird zu "/", ein abschließender Slash bleibt erhalten
  (/a und /a/ sind verschiedene URLs); Query bleibt unverändert, ein leeres "?" entfällt.
"""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import json
import os
import re
import string
import subprocess
import sys
import tempfile
import tomllib
import urllib.error
import urllib.parse
import urllib.request

CLASSES = ("gleichwertig", "teilweise", "ohne Gegenstück")
EXAMPLES = 5
SCHEMA = 1
MAX_PAGES = 10000
LOOPBACK_HOSTS = {"127.0.0.1", "::1"}

# --------------------------------------------------------------------------------------------------
# URL-Normalisierung

_UNRESERVED = set(string.ascii_letters + string.digits + "-._~")


def _norm_percent(text: str) -> str:
    def repl(m: re.Match[str]) -> str:
        ch = chr(int(m.group(1), 16))
        return ch if ch in _UNRESERVED else "%" + m.group(1).upper()

    return re.sub(r"%([0-9A-Fa-f]{2})", repl, text)


def normalize_url(url: str) -> str:
    """Normalisiert eine URL für den Abgleich (Regeln im Modulkopf)."""
    text = url.strip().partition("#")[0]
    try:
        parts = urllib.parse.urlsplit(text)
        port = parts.port
    except ValueError:
        return text
    scheme = parts.scheme.lower()
    host = (parts.hostname or "").lower()
    if ":" in host:
        host = f"[{host}]"
    if port is not None and (scheme, port) not in (("http", 80), ("https", 443)):
        host = f"{host}:{port}"
    path = _norm_percent(parts.path) or "/"
    query = _norm_percent(parts.query)
    return urllib.parse.urlunsplit((scheme, host, path, query, ""))


# --------------------------------------------------------------------------------------------------
# Namen der Exportdateien (Regeln aus OUT-008, so wie os-export sie anwendet)

MAX_STEM_LEN = 120


def sanitize_name(text: str) -> str:
    out: list[str] = []
    for ch in text:
        c = ch.lower() if ch.isascii() else "_"
        if not (("a" <= c <= "z") or ("0" <= c <= "9") or c in "-._"):
            c = "_"
        if c == "_" and out and out[-1] == "_":
            continue
        out.append(c)
    return "".join(out).strip("_.-")


# Standardschwellen (os-model, CrawlConfig::default); Werte aus der Konfigurationsdatei überschreiben sie.
DEFAULT_LIMITS: dict[str, dict[str, int]] = {
    "title": {"max_chars": 60, "min_chars": 30, "max_px": 561, "min_px": 200},
    "meta_description": {"max_chars": 155, "min_chars": 70, "max_px": 985, "min_px": 400},
    "url": {"max_chars": 115},
    "h1": {"max_chars": 70},
    "h2": {"max_chars": 70},
    "image": {"max_kb": 100, "alt_max_chars": 100},
}
_PLACEHOLDER = re.compile(r"\{([^{}]*)\}")


def load_limits(osconfig_path: str | None = None) -> dict[str, dict[str, int]]:
    """Standardschwellen, überlagert mit den Abschnitten title, meta_description, url, h1, h2, image der osconfig."""
    limits = {section: dict(values) for section, values in DEFAULT_LIMITS.items()}
    if osconfig_path:
        with open(osconfig_path, "rb") as fh:
            doc = tomllib.load(fh)
        for section, values in limits.items():
            over = doc.get(section)
            if isinstance(over, dict):
                for key in values:
                    if isinstance(over.get(key), int) and not isinstance(over.get(key), bool):
                        values[key] = over[key]
    return limits


def resolve_placeholders(text: str, limits: dict[str, dict[str, int]] | None = None) -> str:
    """Ersetzt {abschnitt.schluessel} durch den wirksamen Wert, wie die Binärdatei beim Dateinamen; Unbekanntes bleibt stehen."""
    limits = DEFAULT_LIMITS if limits is None else limits

    def repl(m: re.Match[str]) -> str:
        section, _, key = m.group(1).partition(".")
        value = limits.get(section, {}).get(key)
        return m.group(0) if value is None else str(value)

    return _PLACEHOLDER.sub(repl, text)


def export_file_name(tab_filter: str, ext: str = "csv", limits: dict[str, dict[str, int]] | None = None) -> str:
    """Dateiname eines Tab-Exports; Platzhalter werden mit den wirksamen Schwellen aufgelöst.

    Namen über 120 Zeichen werden nicht unterstützt (Hash-Kürzung)."""
    stem = sanitize_name(resolve_placeholders(tab_filter, limits)) or "export"
    if len(stem) > MAX_STEM_LEN:
        raise ValueError(f"Exportname zu lang für die einfache Zuordnung: {tab_filter}")
    return f"{stem}.{ext}"


def find_export_file(export_dir: str, tab_filter: str, limits: dict[str, dict[str, int]] | None = None) -> str | None:
    """Pfad der Exportdatei: erst der berechnete Name, sonst bei Platzhaltern eine eindeutige Mustersuche im Ordner."""
    path = os.path.join(export_dir, export_file_name(tab_filter, limits=limits))
    if os.path.isfile(path):
        return path
    if not _PLACEHOLDER.search(tab_filter):
        return None
    # Rückfall: jeder Platzhalter steht für eine beliebige Zahl; nur ein einziger Treffer gilt.
    marker = "placeholderx"
    stem = sanitize_name(_PLACEHOLDER.sub(marker, tab_filter))
    pattern = re.compile("^" + re.escape(stem).replace(marker, "[0-9]+") + r"\.csv$")
    try:
        names = sorted(os.listdir(export_dir))
    except OSError:
        return None
    hits = [n for n in names if pattern.match(n)]
    return os.path.join(export_dir, hits[0]) if len(hits) == 1 else None


# --------------------------------------------------------------------------------------------------
# Zuordnungstabelle

def load_rule_map(path: str) -> dict:
    with open(path, "rb") as fh:
        return tomllib.load(fh)


def validate_rule_map(data: dict, known_filters: set[str] | None = None) -> list[str]:
    """Gibt eine Liste von Problemen zurück (leer = gültig)."""
    problems: list[str] = []
    if data.get("schema") != SCHEMA:
        problems.append(f"schema muss {SCHEMA} sein")
    export = data.get("export", {})
    listed = list(export.get("base", [])) + list(export.get("extra", []))
    seen_ids: set[str] = set()
    reasons: dict[str, str] = {}
    for i, rule in enumerate(data.get("rule", [])):
        where = f"rule[{i}]"
        rid = rule.get("autonomo")
        if not isinstance(rid, str) or not rid.strip():
            problems.append(f"{where}: autonomo fehlt")
            continue
        where = f"rule {rid}"
        if rid in seen_ids:
            problems.append(f"{where}: doppelt")
        seen_ids.add(rid)
        cls = rule.get("class")
        if cls not in CLASSES:
            problems.append(f"{where}: class muss eines von {CLASSES} sein")
        filters = rule.get("openspider", [])
        if not isinstance(filters, list):
            problems.append(f"{where}: openspider muss eine Liste sein")
            filters = []
        if cls == "ohne Gegenstück" and filters:
            problems.append(f"{where}: ohne Gegenstück darf keine openspider-Kennung haben")
        if cls in ("gleichwertig", "teilweise") and not filters:
            problems.append(f"{where}: {cls} braucht mindestens eine openspider-Kennung")
        reason = rule.get("reason")
        if not isinstance(reason, str) or not reason.strip():
            problems.append(f"{where}: reason fehlt")
        elif reason.strip() in reasons:
            problems.append(f"{where}: reason ist identisch mit der von {reasons[reason.strip()]}")
        else:
            reasons[reason.strip()] = rid
        for f in filters:
            if ":" not in f:
                problems.append(f"{where}: {f!r} ist keine Tab:Filter-Kennung")
            elif known_filters is not None and f.lower() not in known_filters:
                problems.append(f"{where}: {f!r} steht nicht in den Filter-Dateien")
    if known_filters is not None:
        for f in listed:
            if f.lower() not in known_filters:
                problems.append(f"export: {f!r} steht nicht in den Filter-Dateien")
    return problems


def export_filters(data: dict) -> list[str]:
    """Alle zu exportierenden Tab:Filter-Kennungen: base, extra und alle zugeordneten, ohne Duplikate."""
    export = data.get("export", {})
    names = list(export.get("base", [])) + list(export.get("extra", []))
    for rule in data.get("rule", []):
        names.extend(rule.get("openspider", []))
    out: list[str] = []
    seen: set[str] = set()
    for n in names:
        if n.lower() not in seen:
            seen.add(n.lower())
            out.append(n)
    return out


def known_filter_ids(*toml_paths: str) -> set[str]:
    """Kleingeschriebene Tab:Filter-Kennungen aus M1-/M2-filters.toml."""
    ids: set[str] = set()
    for p in toml_paths:
        with open(p, "rb") as fh:
            doc = tomllib.load(fh)
        for tab in doc.get("tab", []):
            for flt in tab.get("filters", []):
                ids.add(f"{tab['id']}:{flt['id']}".lower())
    return ids


# --------------------------------------------------------------------------------------------------
# Daten laden

class ExportSet(dict):
    """Normalisierte Adressen je Tab:Filter; unreadable nennt Exporte, die vorhanden, aber nicht lesbar sind."""

    def __init__(self) -> None:
        super().__init__()
        self.unreadable: dict[str, str] = {}


def load_os_exports(export_dir: str, filters: list[str], limits: dict[str, dict[str, int]] | None = None) -> ExportSet:
    """Normalisierte Adressen je Tab:Filter aus den CSV-Dateien.

    None = Datei fehlt; None mit Eintrag in .unreadable = Datei da, aber ohne Spalte 'Address' oder nicht lesbar
    (wird als Problem gemeldet und nie als leere Menge gezählt)."""
    result = ExportSet()
    for tf in filters:
        path = find_export_file(export_dir, tf, limits)
        if path is None:
            result[tf] = None
            continue
        try:
            with open(path, newline="", encoding="utf-8-sig") as fh:
                reader = csv.DictReader(fh)
                if reader.fieldnames is None or "Address" not in reader.fieldnames:
                    result[tf] = None
                    result.unreadable[tf] = "Spalte 'Address' fehlt"
                    continue
                result[tf] = [normalize_url(row["Address"]) for row in reader if row.get("Address")]
        except (OSError, UnicodeDecodeError, csv.Error) as e:
            result[tf] = None
            result.unreadable[tf] = f"nicht lesbar ({type(e).__name__})"
    return result


def parse_time(value: str | None) -> dt.datetime | None:
    if not value:
        return None
    try:
        t = dt.datetime.fromisoformat(value.strip().replace("Z", "+00:00"))
    except ValueError:
        return None
    return t if t.tzinfo else t.replace(tzinfo=dt.timezone.utc)


# --------------------------------------------------------------------------------------------------
# Vergleich

def _stats(a: set[str], o: set[str]) -> dict:
    inter = a & o
    union = a | o
    only_a = sorted(a - o)
    only_o = sorted(o - a)
    return {
        "count_autonomo": len(a),
        "count_openspider": len(o),
        "intersection": len(inter),
        "only_autonomo": len(only_a),
        "only_openspider": len(only_o),
        "jaccard": round(len(inter) / len(union), 4) if union else 1.0,
        "examples_only_autonomo": only_a[:EXAMPLES],
        "examples_only_openspider": only_o[:EXAMPLES],
    }


def compare_site(
    site: str,
    os_data: dict[str, list[str] | None],
    autonomo: dict,
    rule_map: dict,
    os_finished: str | None,
) -> dict:
    """Vergleicht je Regel; os_data: Ausgabe von load_os_exports, autonomo: normalisierte Datei."""
    findings = {rid: {normalize_url(u) for u in urls} for rid, urls in autonomo.get("findings", {}).items()}
    gap = None
    t_os = parse_time(os_finished)
    t_a = parse_time(autonomo.get("crawl", {}).get("finished_at"))
    if t_os and t_a:
        gap = int(abs((t_os - t_a).total_seconds()))

    rules_out = []
    mapped_os: set[str] = set()
    mapped_rules: set[str] = set()
    for rule in rule_map.get("rule", []):
        rid = rule["autonomo"]
        mapped_rules.add(rid)
        a = findings.get(rid, set())
        entry = {
            "autonomo": rid,
            "class": rule["class"],
            "reason": rule["reason"],
            "openspider": list(rule.get("openspider", [])),
            "autonomo_present": rid in findings,
        }
        if rule["class"] == "ohne Gegenstück":
            entry.update({"comparable": False, "count_autonomo": len(a)})
            rules_out.append(entry)
            continue
        unreadable = getattr(os_data, "unreadable", {})
        bad = [f for f in rule["openspider"] if f in unreadable]
        missing = [f for f in rule["openspider"] if os_data.get(f) is None and f not in unreadable]
        o: set[str] = set()
        for f in rule["openspider"]:
            mapped_os.add(f.lower())
            o.update(os_data.get(f) or [])
        entry.update({"comparable": not missing and not bad, "missing_exports": missing, "unreadable_exports": bad})
        entry.update(_stats(a, o))
        rules_out.append(entry)

    return {
        "schema": SCHEMA,
        "site": site,
        "autonomo_crawl": autonomo.get("crawl", {}),
        "autonomo_source": autonomo.get("source"),
        "openspider_finished_at": os_finished,
        "time_gap_s": gap,
        "unreadable_exports": dict(sorted(getattr(os_data, "unreadable", {}).items())),
        "rules": rules_out,
        "autonomo_unmapped": {
            rid: len(urls) for rid, urls in sorted(findings.items()) if rid not in mapped_rules
        },
        "openspider_unmapped": {
            tf: (None if urls is None else len(set(urls)))
            for tf, urls in sorted(os_data.items())
            if tf.lower() not in mapped_os and tf not in getattr(os_data, "unreadable", {})
        },
    }


def render_markdown(result: dict) -> str:
    lines = [f"# Schattenvergleich {result['site']}", ""]
    crawl = result.get("autonomo_crawl", {})
    lines.append(f"- Autonomo-Crawl: {crawl.get('id', '?')} (beendet {crawl.get('finished_at', '?')}, Quelle {result.get('autonomo_source', '?')})")
    lines.append(f"- OpenSpider-Crawl beendet: {result.get('openspider_finished_at') or '?'}")
    gap = result.get("time_gap_s")
    lines.append(f"- Zeitabstand der Crawls: {'?' if gap is None else f'{gap} s ({gap / 3600:.1f} h)'}")
    metrics = result.get("metrics") or {}
    if "urls_total" in metrics:
        lines.append(f"- URLs gecrawlt (Zeilen Internal:All): {metrics['urls_total']}"
                     f" (laut Fortschrittsdatei: {metrics.get('urls_crawled', '?')})")
    lines.append("")
    lines.append("| Regel | Klasse | OpenSpider | Autonomo | OS | Schnitt | nur A | nur OS | Jaccard |")
    lines.append("|---|---|---|---|---|---|---|---|---|")
    for r in result["rules"]:
        if not r.get("comparable", True) and r["class"] != "ohne Gegenstück":
            reason = []
            if r.get("missing_exports"):
                reason.append(f"Export fehlt: {', '.join(r['missing_exports'])}")
            if r.get("unreadable_exports"):
                reason.append(f"Export unlesbar: {', '.join(r['unreadable_exports'])}")
            lines.append(f"| {r['autonomo']} | {r['class']} | {', '.join(r['openspider'])} | – | – | – | – | – | {'; '.join(reason)} |")
        elif r["class"] == "ohne Gegenstück":
            lines.append(f"| {r['autonomo']} | {r['class']} | – | {r['count_autonomo']} | – | – | – | – | – |")
        else:
            lines.append(
                f"| {r['autonomo']} | {r['class']} | {', '.join(r['openspider'])} | {r['count_autonomo']} | "
                f"{r['count_openspider']} | {r['intersection']} | {r['only_autonomo']} | {r['only_openspider']} | {r['jaccard']:.4f} |"
            )
    if result.get("unreadable_exports"):
        lines += ["", "## Unlesbare Exporte (Problem, nicht vergleichbar)", ""]
        lines += [f"- {tf}: {why}" for tf, why in result["unreadable_exports"].items()]
    deviations = [r for r in result["rules"] if r.get("comparable") and r.get("jaccard", 1.0) < 1.0]
    lines += ["", "## Abweichungen", ""]
    if not deviations:
        lines += ["Keine Abweichungen bei den vergleichbaren Regeln.", ""]
    for r in deviations:
        lines.append(f"### {r['autonomo']} ({r['class']}, Jaccard {r['jaccard']:.4f})")
        lines.append("")
        lines.append(f"Begründung der Zuordnung: {r['reason']}")
        lines.append("")
        for key, label in (("examples_only_autonomo", "nur Autonomo"), ("examples_only_openspider", "nur OpenSpider")):
            if r[key]:
                lines.append(f"- {label} (bis {EXAMPLES} Beispiele):")
                lines.extend(f"  - {u}" for u in r[key])
        lines.append("")
    if result["autonomo_unmapped"]:
        lines += ["## Autonomo-Regeln ohne Zuordnung", ""]
        lines += [f"- {rid}: {n}" for rid, n in result["autonomo_unmapped"].items()]
        lines.append("")
    if result["openspider_unmapped"]:
        lines += ["## OpenSpider-Exporte ohne Zuordnung", ""]
        lines += [f"- {tf}: {'Export fehlt' if n is None else n}" for tf, n in result["openspider_unmapped"].items()]
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


# --------------------------------------------------------------------------------------------------
# Messwerte

def compute_metrics(progress_lines: list[str], run_meta: dict, window_s: float = 10.0) -> dict:
    """Mittlere Rate, Höchstrate (gemeldet und im gleitenden Fenster) und Spitzen-RSS."""
    points: list[tuple[float, int]] = []
    reported_max = 0.0
    for line in progress_lines:
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            obj = json.loads(line)
        except ValueError:
            continue
        if obj.get("phase") != "crawl" or "crawled" not in obj or "elapsed_s" not in obj:
            continue
        points.append((float(obj["elapsed_s"]), int(obj["crawled"])))
        reported_max = max(reported_max, float(obj.get("rate") or 0.0))
    mean = 0.0
    mean_low = 0.0
    window_max = 0.0
    if points:
        elapsed, crawled = points[-1]
        mean = crawled / elapsed if elapsed > 0 else 0.0
        # elapsed_s kommt in ganzen Sekunden (abgeschnitten): die wahre Dauer liegt in [elapsed, elapsed + 1).
        mean_low = crawled / (elapsed + 1.0)
        i = 0
        for j in range(len(points)):
            while i < j and points[j][0] - points[i + 1][0] >= window_s:
                i += 1
            span = points[j][0] - points[i][0]
            if span >= window_s:
                window_max = max(window_max, (points[j][1] - points[i][1]) / span)
    return {
        "urls_crawled": points[-1][1] if points else 0,
        "crawl_elapsed_s": points[-1][0] if points else 0,
        "mean_rate": round(mean, 3),
        "mean_rate_low": round(mean_low, 3),
        "mean_rate_coarse": bool(points) and points[-1][0] < 60,
        "max_rate_reported": round(reported_max, 3),
        "max_rate_window": round(window_max, 3),
        "window_s": window_s,
        "peak_rss_kb": run_meta.get("peak_rss_kb"),
        "exit_code": run_meta.get("exit_code"),
        "duration_s": run_meta.get("duration_s"),
    }


# --------------------------------------------------------------------------------------------------
# Autonomo lesen

def read_env_value(path: str, var: str) -> str | None:
    """Liest VAR aus einer Datei bzw. den Dateien eines Ordners (nicht rekursiv); gibt den Wert nie aus."""
    candidates = [path]
    if os.path.isdir(path):
        candidates = [os.path.join(path, n) for n in sorted(os.listdir(path))]
    pattern = re.compile(rf"^\s*(?:export\s+)?{re.escape(var)}\s*=\s*(.*?)\s*$")
    for file in candidates:
        if not os.path.isfile(file):
            continue
        try:
            with open(file, encoding="utf-8", errors="replace") as fh:
                for raw in fh:
                    if raw.lstrip().startswith("#"):
                        continue
                    m = pattern.match(raw)
                    if m:
                        value = m.group(1)
                        if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
                            value = value[1:-1]
                        return value or None
        except OSError:
            continue
    return None


def _first(obj: dict, names: list[str]):
    for n in names:
        if n in obj and obj[n] not in (None, ""):
            return obj[n]
    return None


def _outline(value, depth: int = 0) -> object:
    """Struktur ohne Werte: Schlüssel, Typen, Listenlängen."""
    if isinstance(value, dict):
        if depth >= 3:
            return f"object({len(value)})"
        return {k: _outline(v, depth + 1) for k, v in sorted(value.items())}
    if isinstance(value, list):
        return [f"list({len(value)})", _outline(value[0], depth + 1)] if value else "list(0)"
    return type(value).__name__


def extract_findings(payload, rule_keys: list[str], url_keys: list[str]) -> dict[str, list[str]]:
    """Wandelt eine Befund-Antwort in {rule_id: [url, …]}; unterstützt Zeilenlisten und Zuordnungen."""
    out: dict[str, list[str]] = {}

    def add(rid, url):
        if isinstance(rid, str) and rid and isinstance(url, str) and url:
            out.setdefault(rid, []).append(url)

    if isinstance(payload, list):
        for row in payload:
            if isinstance(row, dict):
                add(_first(row, rule_keys), _first(row, url_keys))
        return out
    if isinstance(payload, dict):
        for wrapper in ("befunde", "findings", "items", "data", "rows"):
            if isinstance(payload.get(wrapper), (list, dict)):
                return extract_findings(payload[wrapper], rule_keys, url_keys)
        if payload and all(isinstance(v, list) for v in payload.values()):
            for rid, urls in payload.items():
                for u in urls:
                    add(rid, u if isinstance(u, str) else (_first(u, url_keys) if isinstance(u, dict) else None))
            return out
    raise ValueError(f"unbekannte Form der Befund-Antwort: {json.dumps(_outline(payload))}")


def _crawl_candidates(payload, fields: dict, site: str) -> list[dict]:
    rows = payload
    if isinstance(payload, dict):
        for wrapper in ("crawls", "items", "data", "rows"):
            if isinstance(payload.get(wrapper), list):
                rows = payload[wrapper]
                break
    if not isinstance(rows, list):
        raise ValueError(f"unbekannte Form der Crawl-Liste: {json.dumps(_outline(payload))}")
    out = []
    for row in rows:
        if not isinstance(row, dict):
            continue
        ident = _first(row, fields["id"])
        finished = _first(row, fields["finished_at"])
        scope = " ".join(str(_first(row, [k]) or "") for k in fields["site"])
        status = str(_first(row, fields["status"]) or "")
        if ident is None or site not in scope:
            continue
        if status and status.lower() not in fields["done_values"]:
            continue
        out.append({"id": str(ident), "started_at": _first(row, fields["started_at"]), "finished_at": finished})
    return out


def pick_nearest(crawls: list[dict], near: str) -> dict | None:
    """Der abgeschlossene Crawl mit dem geringsten Zeitabstand zu `near`."""
    t0 = parse_time(near)
    best, best_gap = None, None
    for c in crawls:
        t = parse_time(c.get("finished_at"))
        if t is None or t0 is None:
            continue
        gap = abs((t - t0).total_seconds())
        if best_gap is None or gap < best_gap:
            best, best_gap = c, gap
    return best


API_DEFAULTS = {
    "base_url": "http://127.0.0.1:7781/api/extern",
    "env_path": "/etc/autonomo-seo",
    "token_var": "AUTONOMO_EXTERN_TOKEN",
    "crawl_list_path": "onpage/crawls",
    "findings_path": "onpage/crawls/{crawl}/befunde",
    "page_size": 0,
    "timeout_s": 60,
}
FIELD_DEFAULTS = {
    "id": ["id", "crawl_id", "session_id"],
    "started_at": ["started_at", "gestartet", "start"],
    "finished_at": ["finished_at", "beendet", "ended_at", "abgeschlossen_am", "completed_at"],
    "site": ["domain", "host", "start_url", "url", "projekt", "project"],
    "status": ["status", "state", "zustand"],
    "done_values": ["fertig", "done", "completed", "finished", "abgeschlossen"],
    "rule": ["rule_id", "regel", "regel_id", "rule"],
    "url": ["url", "adresse", "address"],
}


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    """Lehnt jede Weiterleitung ab, damit der Bearer-Token nie an ein anderes Ziel als die geprüfte Loopback-URL geht."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):  # noqa: D401, ARG002
        raise urllib.error.HTTPError(req.full_url, code, "Weiterleitung abgelehnt", headers, fp)


def _opener() -> urllib.request.OpenerDirector:
    # build_opener() mit eigener Redirect-Klasse ersetzt den Standard-HTTPRedirectHandler.
    return urllib.request.build_opener(_NoRedirect)


def _api_get(base: str, path: str, token: str, timeout: int, query: dict | None = None):
    url = base.rstrip("/") + "/" + path.lstrip("/")
    if query:
        url += "?" + urllib.parse.urlencode(query)
    host = urllib.parse.urlsplit(url).hostname
    if host not in LOOPBACK_HOSTS:
        raise SystemExit("Abbruch: der Autonomo-Zugang ist nur über Loopback erlaubt")
    req = urllib.request.Request(url, method="GET", headers={"Authorization": f"Bearer {token}", "Accept": "application/json"})
    try:
        with _opener().open(req, timeout=timeout) as resp:  # nur Loopback, geprüft; keine Weiterleitungen
            return json.load(resp)
    except urllib.error.HTTPError as e:
        raise SystemExit(f"Autonomo-API: HTTP {e.code} für {path}") from None
    except (urllib.error.URLError, OSError, ValueError) as e:
        raise SystemExit(f"Autonomo-API nicht erreichbar oder ungültige Antwort für {path}: {type(e).__name__}") from None


def _load_source_config(path: str) -> dict:
    with open(path, "rb") as fh:
        return tomllib.load(fh)


def fetch_via_api(site: str, near: str, cfg: dict) -> dict:
    api = {**API_DEFAULTS, **cfg.get("api", {})}
    fields = {**FIELD_DEFAULTS, **cfg.get("api", {}).get("fields", {})}
    if not api.get("verified"):
        raise SystemExit("Abbruch: [api] verified = false – Pfade und Feldnamen erst mit 'discover' lesend prüfen")
    token = read_env_value(api["env_path"], api["token_var"])
    if not token:
        raise SystemExit("Abbruch: Autonomo-Token in der bestehenden Konfiguration nicht gefunden")
    listing = _api_get(api["base_url"], api["crawl_list_path"], token, api["timeout_s"])
    crawl = pick_nearest(_crawl_candidates(listing, fields, site), near)
    if crawl is None:
        raise SystemExit(f"Abbruch: kein abgeschlossener Autonomo-Crawl für {site} über die API erreichbar")
    findings: dict[str, list[str]] = {}
    page, offset = int(api["page_size"]), 0
    previous: dict[str, list[str]] | None = None
    for _ in range(MAX_PAGES):
        query = {"limit": page, "offset": offset} if page else None
        payload = _api_get(api["base_url"], api["findings_path"].format(crawl=urllib.parse.quote(crawl["id"], safe="")), token, api["timeout_s"], query)
        chunk = extract_findings(payload, fields["rule"], fields["url"])
        if previous is not None and chunk == previous:
            # Die Schnittstelle ignoriert limit/offset und liefert dieselbe Seite erneut: nicht vervielfachen.
            print("Warnung: identische Antwort auf die Folgeseite, Blättern abgebrochen (limit/offset wirkungslos?)", file=sys.stderr)
            break
        previous = chunk
        for rid, urls in chunk.items():
            findings.setdefault(rid, []).extend(urls)
        rows = sum(len(v) for v in chunk.values())
        if not page or rows < page:
            break
        offset += page
    else:
        raise SystemExit(f"Abbruch: mehr als {MAX_PAGES} Seiten Befunde, Schnittstelle prüfen")
    return {"schema": SCHEMA, "site": site, "source": "api", "crawl": crawl, "findings": findings}


_SELECT_ONLY = re.compile(r"^\s*(select|with)\b", re.IGNORECASE)
_FORBIDDEN = re.compile(r"\b(insert|update|delete|drop|alter|create|truncate|grant|revoke|copy|call|do|vacuum|set|lock)\b", re.IGNORECASE)
_SAFE_VALUE = re.compile(r"^[A-Za-z0-9_.:-]+$")


def check_select_only(sql: str) -> str:
    """Lässt nur eine einzelne SELECT-Anweisung zu (zusätzlich zur schreibgeschützten Sitzung)."""
    body = sql.strip().rstrip(";").strip()
    if ";" in body or not _SELECT_ONLY.match(body) or _FORBIDDEN.search(body):
        raise SystemExit("Abbruch: nur eine einzelne SELECT-Anweisung ist erlaubt")
    return body


def fetch_via_psql(site: str, near: str, cfg: dict) -> dict:
    pg = cfg.get("psql", {})
    if not pg.get("verified"):
        raise SystemExit("Abbruch: [psql] verified = false – Aufruf und Abfragen erst lesend gegen das Schema prüfen")
    if not _SAFE_VALUE.match(site):
        raise SystemExit("Abbruch: ungültiger Site-Name")
    env = {**os.environ, "PGOPTIONS": "-c default_transaction_read_only=on"}
    if pg.get("password_var"):
        pw = read_env_value(pg.get("env_path", "/etc/autonomo-seo"), pg["password_var"])
        if pw:
            env["PGPASSWORD"] = pw

    def query(sql: str) -> list[list[str]]:
        argv = [a.replace("{query}", check_select_only(sql)) for a in pg["argv"]]
        proc = subprocess.run(argv, env=env, capture_output=True, text=True, timeout=300, check=False)
        if proc.returncode != 0:
            raise SystemExit(f"Abbruch: psql endete mit Code {proc.returncode}")
        return [line.split("\t") for line in proc.stdout.splitlines() if line.strip()]

    crawls = [
        {"id": r[0], "started_at": r[1] if len(r) > 1 else None, "finished_at": r[2] if len(r) > 2 else None}
        for r in query(pg["crawl_sql"].replace("{site}", site))
    ]
    crawl = pick_nearest(crawls, near)
    if crawl is None:
        raise SystemExit(f"Abbruch: kein abgeschlossener Autonomo-Crawl für {site} in der Datenbank")
    if not _SAFE_VALUE.match(crawl["id"]):
        raise SystemExit("Abbruch: unerwartete Crawl-Kennung")
    findings: dict[str, list[str]] = {}
    for row in query(pg["findings_sql"].replace("{crawl}", crawl["id"])):
        if len(row) >= 2:
            findings.setdefault(row[0], []).append(row[1])
    return {"schema": SCHEMA, "site": site, "source": "psql", "crawl": crawl, "findings": findings}


# --------------------------------------------------------------------------------------------------
# Befehle

_SAFE_SITE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9.-]*$")


def check_site(site: str) -> str:
    """Site-Kennung wird zu Pfaden: nur Hostname-Zeichen, kein '..' (compare.sh ruft das als root auf)."""
    if not _SAFE_SITE.match(site) or ".." in site:
        raise SystemExit(f"Abbruch: ungültige Site-Kennung {site!r}")
    return site


def check_inside(path: str, base: str | None, what: str) -> str:
    """Mit --base: der aufgelöste Pfad (Symlinks, '..') muss unterhalb des Basisordners liegen."""
    if base is None:
        return path
    real_base = os.path.realpath(base)
    real = os.path.realpath(path)
    if real != real_base and not real.startswith(real_base + os.sep):
        raise SystemExit(f"Abbruch: {what} liegt außerhalb von {base}")
    return path


def _write_json(path: str, obj) -> None:
    """Atomar über eine neue Datei im selben Ordner: ein dort abgelegter Symlink wird ersetzt, nie befolgt."""
    directory = os.path.dirname(os.path.abspath(path))
    os.makedirs(directory, exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=directory, prefix=".tmp-", suffix=".json")
    try:
        umask = os.umask(0)
        os.umask(umask)
        os.fchmod(fd, 0o666 & ~umask)
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            json.dump(obj, fh, ensure_ascii=False, indent=1, sort_keys=True)
            fh.write("\n")
        os.replace(tmp, path)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise


def cmd_exports(args) -> int:
    data = load_rule_map(args.rule_map)
    names = export_filters(data)
    print(",".join(n.replace(",", "\\,") for n in names))
    return 0


def cmd_validate(args) -> int:
    data = load_rule_map(args.rule_map)
    known = known_filter_ids(*args.filters) if args.filters else None
    problems = validate_rule_map(data, known)
    for p in problems:
        print(p, file=sys.stderr)
    return 1 if problems else 0


def cmd_fetch(args) -> int:
    check_site(args.site)
    check_inside(args.out, args.base, "--out")
    cfg = _load_source_config(args.config) if args.config and os.path.isfile(args.config) else {}
    if args.source == "file":
        with open(args.file, encoding="utf-8") as fh:
            result = json.load(fh)
    elif args.source == "api":
        result = fetch_via_api(args.site, args.near, cfg)
    else:
        result = fetch_via_psql(args.site, args.near, cfg)
    _write_json(args.out, result)
    print(f"{args.site}: Autonomo-Crawl {result['crawl'].get('id')} ({result['source']}), "
          f"{sum(len(v) for v in result['findings'].values())} Befundzeilen in {len(result['findings'])} Regeln")
    return 0


def cmd_discover(args) -> int:
    cfg = _load_source_config(args.config)
    api = {**API_DEFAULTS, **cfg.get("api", {})}
    token = read_env_value(api["env_path"], api["token_var"])
    if not token:
        raise SystemExit("Abbruch: Autonomo-Token in der bestehenden Konfiguration nicht gefunden")
    path = args.path or api["crawl_list_path"]
    print(json.dumps(_outline(_api_get(api["base_url"], path, token, api["timeout_s"])), indent=1, ensure_ascii=False))
    return 0


def cmd_compare(args) -> int:
    check_site(args.site)
    check_inside(args.out, args.base, "--out")
    rule_map = load_rule_map(args.rule_map)
    problems = validate_rule_map(rule_map)
    if problems:
        for p in problems:
            print(p, file=sys.stderr)
        return 1
    meta = {}
    if args.run_meta and os.path.isfile(args.run_meta):
        with open(args.run_meta, encoding="utf-8") as fh:
            meta = json.load(fh)
    with open(args.autonomo, encoding="utf-8") as fh:
        autonomo = json.load(fh)
    filters = export_filters(rule_map)
    limits = load_limits(args.osconfig)
    os_data = load_os_exports(args.exports, filters, limits)
    result = compare_site(args.site, os_data, autonomo, rule_map, meta.get("finished_at"))
    if args.progress and os.path.isfile(args.progress):
        with open(args.progress, encoding="utf-8", errors="replace") as fh:
            result["metrics"] = compute_metrics(fh.read().splitlines(), meta)
    # "URLs gecrawlt" im Bericht = Zeilen von Internal:All (enthält auch robots-gesperrte URLs mit Status 0);
    # urls_crawled aus der Fortschrittsdatei bleibt als Vergleichswert.
    internal = next((v for k, v in os_data.items() if k.lower() == "internal:all"), None)
    if internal is not None:
        result.setdefault("metrics", {})["urls_total"] = len(internal)
    _write_json(os.path.join(args.out, f"{args.site}.json"), result)
    with open(os.path.join(args.out, f"{args.site}.md"), "w", encoding="utf-8") as fh:
        fh.write(render_markdown(result))
    print(f"{args.site}: {len(result['rules'])} Regeln verglichen, Bericht in {args.out}")
    return 0


def cmd_metrics(args) -> int:
    meta = {}
    if args.run_meta and os.path.isfile(args.run_meta):
        with open(args.run_meta, encoding="utf-8") as fh:
            meta = json.load(fh)
    with open(args.progress, encoding="utf-8", errors="replace") as fh:
        print(json.dumps(compute_metrics(fh.read().splitlines(), meta), indent=1))
    return 0


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = p.add_subparsers(dest="cmd", required=True)

    s = sub.add_parser("exports", help="Tab:Filter-Liste für --export-tabs")
    s.add_argument("--rule-map", required=True)
    s.set_defaults(fn=cmd_exports)

    s = sub.add_parser("validate", help="Zuordnungstabelle prüfen")
    s.add_argument("--rule-map", required=True)
    s.add_argument("--filters", nargs="*", default=[], help="M1-/M2-filters.toml zum Abgleich der Kennungen")
    s.set_defaults(fn=cmd_validate)

    s = sub.add_parser("fetch-autonomo", help="Autonomo-Befunde lesend holen")
    s.add_argument("--site", required=True)
    s.add_argument("--near", required=True, help="Zeitpunkt (ISO 8601), dem der Autonomo-Crawl am nächsten liegen soll")
    s.add_argument("--source", choices=("api", "psql", "file"), default="api")
    s.add_argument("--config", help="etc/autonomo-source.toml")
    s.add_argument("--file", help="bei --source file: bereits normalisierte Datei")
    s.add_argument("--out", required=True)
    s.add_argument("--base", help="Basisordner (var): --out muss darunter liegen (compare.sh setzt es immer)")
    s.set_defaults(fn=cmd_fetch)

    s = sub.add_parser("discover", help="Struktur einer Autonomo-Antwort anzeigen (ohne Werte)")
    s.add_argument("--config", required=True)
    s.add_argument("--path", help="Pfad relativ zur Basis-URL (Standard: Crawl-Liste)")
    s.set_defaults(fn=cmd_discover)

    s = sub.add_parser("compare", help="Exporte gegen Autonomo-Befunde vergleichen")
    s.add_argument("--site", required=True)
    s.add_argument("--exports", required=True, help="Ordner mit den CSV-Exporten des Laufs")
    s.add_argument("--autonomo", required=True, help="normalisierte Autonomo-Datei")
    s.add_argument("--rule-map", required=True)
    s.add_argument("--run-meta", help="run.json des Laufs")
    s.add_argument("--osconfig", help="Konfiguration des Laufs (Schwellen für Platzhalter in Filternamen; Standard: Standardwerte)")
    s.add_argument("--progress", help="Fortschrittsdatei des Laufs (JSON-Zeilen)")
    s.add_argument("--out", required=True)
    s.add_argument("--base", help="Basisordner (var): --out muss darunter liegen")
    s.set_defaults(fn=cmd_compare)

    s = sub.add_parser("metrics", help="Rate und Spitzen-RSS eines Laufs")
    s.add_argument("--progress", required=True)
    s.add_argument("--run-meta")
    s.set_defaults(fn=cmd_metrics)
    return p


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main())
