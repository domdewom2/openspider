#!/usr/bin/env python3
"""M1-N22: Vergleichstabelle der A/B-Kurzprobe.

Aufruf: ab_table.py <ordner mit NN-<name>.json> [--a A1,A2] [--b B1,B2]
Die Namen der Läufe stehen in den Dateinamen (NN-Name.json); A = origin/main, B = Branch.
Liest `metrics` der Berichte von `os-loadtest run --scenario perf-001`.
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
        loadf = path[:-5] + ".load"
        loads = []
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


def series(d, prefix, n=16):
    return [d.get(f"{prefix}_h{i:02d}", 0) for i in range(n)]


def row(name, r, loads):
    d = r.get("crawler_diag", {})
    seg = r["crawler_segment_urls_per_s"]
    mean = r["crawler_urls_per_s"]
    nb = series(d, "store_batches")
    full = [i for i, n in enumerate(nb) if n >= 100]
    commit = series(d, "store_commit_ns")
    first = commit[0] / nb[0] / 1e6 if nb and nb[0] else None
    last = commit[full[-1]] / nb[full[-1]] / 1e6 if full else None
    end = d.get("engine_w_write_max_ns")
    return {
        "Lauf": name,
        "Mittel URLs/s": mean,
        "Dauer s": r["crawler_seconds"],
        "Abschnitt 10 / schnellster": seg[-1] / max(seg) if seg else None,
        "min..max % Mittel": (min(seg) / mean * 100, max(seg) / mean * 100),
        "RSS MB": r["crawler_peak_rss_bytes"] / 1e6,
        "Stillstand s": r["crawler_max_stall_s"],
        "Commit erster Eimer ms": first,
        "Commit letzter voller Eimer ms": last,
        "Endbatch s": end / 1e9 if end else None,
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
    folder = sys.argv[1]
    runs = load(folder)
    rows = [row(n, r, l) for n, r, l in runs]
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
            return sum(vs) / len(vs) if vs else None
        ma, mb = avg(a, "Mittel URLs/s"), avg(b, "Mittel URLs/s")
        print()
        print(f"Mittel A {ma:.1f}, B {mb:.1f} URLs/s ({(mb / ma - 1) * 100:+.1f} %)")
        print(f"RSS A {avg(a, 'RSS MB'):.1f} MB, B {avg(b, 'RSS MB'):.1f} MB ({avg(b, 'RSS MB') - avg(a, 'RSS MB'):+.1f} MB)")
        print(f"Abschnitt 10 / schnellster: A {avg(a, 'Abschnitt 10 / schnellster'):.2f}, B {avg(b, 'Abschnitt 10 / schnellster'):.2f}")
        print(f"Stillstand: A {avg(a, 'Stillstand s'):.2f} s, B {avg(b, 'Stillstand s'):.2f} s")
        print(f"Endbatch: A {avg(a, 'Endbatch s'):.1f} s, B {avg(b, 'Endbatch s'):.1f} s (nur Schreibzeit des längsten Batches)")


main()
