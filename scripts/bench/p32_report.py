#!/usr/bin/env python3
"""M1-P32: erzeugt aus den summary.json der Schrittordner (probe-p32/<schritt>/summary.json) die Vergleichstabellen
des Berichts (Markdown auf stdout). Kein Produktcode.

Aufruf: scripts/bench/p32_report.py docs/plan/M1-acceptance/loadtest/probe-p32 [schritt ...]
"""
import json
import sys
from pathlib import Path


def load(path):
    try:
        return json.loads(Path(path).read_text())
    except (OSError, ValueError):
        return None


def fmt(v, digits=1, suffix=""):
    if v is None:
        return "-"
    if isinstance(v, float):
        return f"{v:.{digits}f}{suffix}"
    return f"{v}{suffix}"


def ns_to_s(ns):
    return None if ns is None else ns / 1e9


def main(root, steps):
    root = Path(root)
    if not steps:
        steps = sorted(p.name for p in root.iterdir() if (p / "summary.json").exists())
    rows = {s: load(root / s / "summary.json") for s in steps}

    print("### Store-Benchmark `writer_1m_uniform_segments` (1 Mio. Seiten x 30 gleichverteilte Links)\n")
    print("| Schritt | Seiten/s | Abschnitte min/max zum Mittel | größter Commit s | größter Checkpoint s | Endbatch s (Indexaufbau) | Datei MiB | Spitzen-RSS MiB | Load beim Start |")
    print("|---|---:|---|---:|---:|---:|---:|---:|---|")
    for s in steps:
        d = (rows[s] or {}).get("store")
        if not d:
            continue
        print(
            f"| {s} | {fmt(d['pages_per_s'], 0)} | {d['min_max_pct'][0]} % / {d['min_max_pct'][1]} % | {fmt(d['commit_max_s'], 2)} | "
            f"{fmt(d['checkpoint_max_s'], 2)} | {fmt(d['end_batch_s'])} | {d['file_mib']} | {d['peak_rss_mib']} | {d.get('load_at_start') or '-'} |"
        )
    print("\nAbschnittszeiten (s) und Anteil am Mittel:\n")
    print("| Schritt | " + " | ".join(f"A{i}" for i in range(1, 11)) + " |")
    print("|---|" + "---:|" * 10)
    for s in steps:
        d = (rows[s] or {}).get("store")
        if d:
            print(f"| {s} | " + " | ".join(f"{t} s ({p} %)" for t, p in zip(d["segment_s"], d["segment_rel_to_mean_pct"])) + " |")

    for name in ("perf-001", "perf-003"):
        print(f"\n### Kurzprobe `{name}` (250 000 Seiten)\n")
        print("| Schritt | URLs/s | Abschnitte (gespeichert) min/max zum Mittel | längster Stillstand s | Spitzen-RSS MB | store_commit_max s | store_checkpoint_max s | bestanden | Load beim Start |")
        print("|---|---:|---|---:|---:|---:|---:|---|---|")
        for s in steps:
            d = (rows[s] or {}).get(name)
            if not d:
                continue
            seg = d.get("segments")
            seg_text = f"{seg['min_max_pct'][0]} % / {seg['min_max_pct'][1]} %" if seg else "-"
            print(
                f"| {s} | {fmt(d.get('urls_per_s'), 0)} | {seg_text} | {fmt(d.get('max_stall_s'))} | {fmt(d.get('peak_rss_mb'))} | "
                f"{fmt(ns_to_s(d.get('store_commit_max_ns')), 2)} | {fmt(ns_to_s(d.get('store_checkpoint_max_ns')), 2)} | {d.get('passed')} | {d.get('load_at_start') or '-'} |"
            )
        print("\nAbschnitte der gespeicherten Seiten (Prozent des Mittels):\n")
        for s in steps:
            d = (rows[s] or {}).get(name)
            if d and d.get("segments"):
                print(f"- {s}: " + ", ".join(f"{p} %" for p in d["segments"]["rel_pct"]))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2:])
