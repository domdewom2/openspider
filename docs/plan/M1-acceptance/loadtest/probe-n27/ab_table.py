#!/usr/bin/env python3
"""M1-N27: Vergleichstabelle der A/B-Kurzprobe mit den Bestandteilen des Endbatchs.

Aufruf: ab_table.py <ordner mit NN-<name>.json> [<ordner> ...]
Namen der Läufe stehen in den Dateinamen (NN-Name.json); A* = origin/main, B* = Branch.
Liest `metrics` der Berichte von `os-loadtest run --scenario perf-001`. Ältere Berichte (Stand A) haben
`crawler_end_batch_s` nicht; dann gilt der größte Schreibaufruf `engine_w_write_max_ns` (der Endbatch).
"""
import glob
import json
import os
import re
import sys


def load(folder):
    runs = []
    for path in sorted(glob.glob(os.path.join(folder, "*.json"))):
        m = re.match(r"(\d+)-(.+)\.json$", os.path.basename(path))
        if not m:
            continue
        r = json.load(open(path))["metrics"]
        loads = []
        loadf = path[:-5] + ".load"
        if os.path.exists(loadf):
            for line in open(loadf):
                parts = line.split()
                if len(parts) >= 2:
                    try:
                        loads.append(float(parts[1].replace(",", ".")))
                    except ValueError:
                        pass
        runs.append((m.group(2), r, loads))
    return runs


def s(d, key):
    v = d.get(key)
    return v / 1e9 if v else None


def row(name, r, loads):
    d = r.get("crawler_diag", {})
    seg = r.get("crawler_segment_urls_per_s") or []
    mean = r["crawler_urls_per_s"]
    end = r.get("crawler_end_batch_s")
    if end is None:
        end = s(d, "engine_w_write_max_ns")
    return {
        "Lauf": name,
        "Mittel URLs/s": mean,
        "Dauer s": r["crawler_seconds"],
        "min..max % Mittel": (min(seg) / mean * 100, max(seg) / mean * 100) if seg else None,
        "RSS MB": r["crawler_peak_rss_bytes"] / 1e6,
        "Stillstand s": r["crawler_max_stall_s"],
        "Endbatch s": end,
        "davon Daten s": s(d, "store_end_data_ns"),
        "links_target s": s(d, "store_link_index_build_ns"),
        "urls-Indizes s": s(d, "store_url_index_build_ns"),
        "Vorwärmen s": s(d, "store_url_index_prefetch_ns"),
        "Commit s": s(d, "store_end_commit_ns") or s(d, "store_commit_max_ns_h05"),
        "Nachlauf s": r.get("crawler_tail_s"),
        "Verstöße": len(r.get("crawler_violations", [])),
        "Load min..max": (min(loads), max(loads)) if loads else None,
    }


def fmt(v):
    if v is None:
        return "-"
    if isinstance(v, tuple):
        return f"{v[0]:.0f}..{v[1]:.0f}" if v[1] > 20 else f"{v[0]:.1f}..{v[1]:.1f}"
    if isinstance(v, float):
        return f"{v:.2f}" if abs(v) < 10 else f"{v:.1f}"
    return str(v)


def main():
    rows = []
    for folder in sys.argv[1:]:
        rows += [row(n, r, l) for n, r, l in load(folder)]
    keys = list(rows[0].keys())
    print("| " + " | ".join(keys) + " |")
    print("|" + "---|" * len(keys))
    for x in rows:
        print("| " + " | ".join(fmt(x[k]) for k in keys) + " |")
    a = [x for x in rows if x["Lauf"].startswith("A")]
    b = [x for x in rows if x["Lauf"].startswith("B")]
    if a and b:
        def avg(rs, k):
            vs = [x[k] for x in rs if x[k] is not None]
            return sum(vs) / len(vs) if vs else float("nan")

        print()
        print(f"Mittel A {avg(a, 'Mittel URLs/s'):.1f}, B {avg(b, 'Mittel URLs/s'):.1f} URLs/s")
        print(f"RSS A {avg(a, 'RSS MB'):.1f} MB, B {avg(b, 'RSS MB'):.1f} MB ({avg(b, 'RSS MB') - avg(a, 'RSS MB'):+.1f} MB)")
        print(f"Stillstand: A {avg(a, 'Stillstand s'):.2f} s, B {avg(b, 'Stillstand s'):.2f} s")
        print(f"Endbatch: A {avg(a, 'Endbatch s'):.1f} s, B {avg(b, 'Endbatch s'):.1f} s")


main()
