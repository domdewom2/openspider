#!/usr/bin/env python3
"""M1-N22: erzeugt die Tabellen des README aus den Rohberichten unter raw/ (Aufruf im Ordner probe-n22)."""
import glob
import json
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
RAW = os.path.join(HERE, "raw")


def load(path):
    r = json.load(open(path))["metrics"]
    loads = []
    lf = path[:-5] + ".load"
    if os.path.exists(lf):
        for line in open(lf):
            p = line.split()
            if len(p) >= 2:
                try:
                    loads.append(float(p[1].replace(",", ".")))
                except ValueError:
                    pass
    return r, loads


def h(d, name, n=16):
    return [d.get(f"{name}_h{i:02d}", 0) for i in range(n)]


def per(d, name, nb, idx):
    v = h(d, name)
    return [v[i] / nb[i] / 1e6 if nb[i] else 0.0 for i in idx]


def ms3(vals):
    return " / ".join(f"{x:.0f}" for x in vals)


def table_causes():
    rows = [
        ("diag1/01-D0", "Diagnosezähler, Standard", "-"),
        ("diag1/02-D1", "WAL-Rückführung nach dem Commit gezählt", "OS_STORE_WAL_EXPLICIT=1"),
        ("exp1/01-Z0", "Standard (Wiederholung)", "-"),
        ("exp1/02-R1drop", "(a) fünf Zufallsindizes von urls entfernt, am Ende gebaut", "OS_STORE_DROP_URL_INDEXES=1"),
        ("exp1/05-R5dropexp", "(a) + Rückführung getrennt gezählt", "OS_STORE_DROP_URL_INDEXES=1, OS_STORE_WAL_EXPLICIT=1"),
        ("exp1/06-R2noflush", "(b) Nachladen ohne Warten auf writer.flush()", "OS_ENGINE_REFILL_NOFLUSH=1"),
        ("exp1/03-R3wal", "(c) WAL-Schwelle 512 MiB bis 1 GiB", "OS_STORE_WAL_BOUNDS=131072:262144"),
        ("exp1/04-R4bg", "(c) Hintergrund-Checkpoint (K-76)", "OS_STORE_BACKGROUND_CHECKPOINT=1"),
    ]
    print("| Lauf | Variante | Schalter | Mittel URLs/s | Dauer s | Abschnitte min..max % | Commit ms/Batch (Eimer 1/2/3) | WAL-Rückführung ms/Batch | Seiten ins WAL je Batch (Eimer 1/2/3) | Refill-Wartezeit ms | Load |")
    print("|---|---|---|---:|---:|---|---|---|---|---:|---|")
    for key, label, env in rows:
        r, loads = load(os.path.join(RAW, key + ".json"))
        d = r["crawler_diag"]
        nb = h(d, "store_batches")
        idx = [0, 1, 2]
        seg = r["crawler_segment_urls_per_s"]
        mean = r["crawler_urls_per_s"]
        cp = per(d, "store_wal_cp_ns", nb, idx)
        cw = [h(d, "store_cache_write")[i] / nb[i] for i in idx]
        rn = d.get("engine_q_refills", 0)
        wait = d.get("engine_q_refill_wait_ns", 0) / rn / 1e6 if rn else 0
        lo = f"{min(loads):.1f}..{max(loads):.1f}" if loads else "-"
        print(
            f"| {key} | {label} | `{env}` | {mean:.0f} | {r['crawler_seconds']:.0f} | "
            f"{min(seg)/mean*100:.0f}..{max(seg)/mean*100:.0f} | {ms3(per(d,'store_commit_ns',nb,idx))} | "
            f"{ms3(cp) if any(cp) else '-'} | {ms3(cw)} | {wait:.0f} | {lo} |"
        )


def table_groups():
    print("| Lauf | Gruppe | Eimer 1 | Eimer 2 | Eimer 3 |")
    print("|---|---|---:|---:|---:|")
    for key in ("exp1/01-Z0", "exp1/02-R1drop"):
        r, _ = load(os.path.join(RAW, key + ".json"))
        d = r["crawler_diag"]
        nb = h(d, "store_batches")
        for name, label in [
            ("store_build_ns", "Aufbau gesamt"),
            ("store_g_urls_ins_ns", "urls einfügen"),
            ("store_g_urls_upd_ns", "urls aktualisieren"),
            ("store_g_links_ns", "links"),
            ("store_g_counters_ns", "url_counters"),
            ("store_g_queue_ns", "queue"),
            ("store_g_rest_ns", "Rest"),
            ("store_commit_ns", "Commit"),
        ]:
            print(f"| {key} | {label} | " + " | ".join(f"{x:.1f}" for x in per(d, name, nb, [0, 1, 2])) + " |")


def one_million(key, title):
    r, loads = load(os.path.join(RAW, key + ".json"))
    d = r["crawler_diag"]
    seg = r["crawler_segment_urls_per_s"]
    mean = r["crawler_urls_per_s"]
    nb = h(d, "store_batches")
    used = [i for i, n in enumerate(nb) if n]
    print(f"**{title}** (`{key}`): Mittel {mean:.1f} URLs/s, Dauer {r['crawler_seconds']:.1f} s, Spitzen-RSS {r['crawler_peak_rss_bytes']/1e6:.1f} MB, "
          f"längster Stillstand {r['crawler_max_stall_s']:.2f} s, Datei {r['crawler_file_bytes']/1e6:.0f} MB, Load {min(loads):.1f}..{max(loads):.1f}.")
    print()
    print("| Abschnitt | " + " | ".join(str(i + 1) for i in range(len(seg))) + " |")
    print("|---|" + "---:|" * len(seg))
    print("| URLs/s | " + " | ".join(f"{x:.0f}" for x in seg) + " |")
    print("| % des Mittels | " + " | ".join(f"{x/mean*100:.0f}" for x in seg) + " |")
    print()
    print("| Eimer (je 100 Batches) | " + " | ".join(f"{i}" for i in used) + " |")
    print("|---|" + "---:|" * len(used))
    for name, label in [
        ("store_commit_ns", "Commit ms/Batch"),
        ("store_build_ns", "Aufbau ms/Batch"),
        ("store_g_urls_upd_ns", "davon urls aktualisieren"),
        ("store_g_links_ns", "davon links"),
        ("store_g_counters_ns", "davon url_counters"),
    ]:
        print(f"| {label} | " + " | ".join(f"{x:.0f}" for x in per(d, name, nb, used)) + " |")
    print("| Batches | " + " | ".join(str(nb[i]) for i in used) + " |")
    print()
    print(f"Endbatch (längster Schreibaufruf `engine_w_write_max_ns`): {d['engine_w_write_max_ns']/1e9:.1f} s, davon Index `links_target` {d['store_link_index_build_ns']/1e9:.1f} s, "
          f"fünf Indizes von `urls` {d.get('store_url_index_build_ns',0)/1e9:.1f} s (längster {d.get('store_url_index_build_max_ns',0)/1e9:.1f} s; Vorwärmen: "
          f"{d.get('store_url_index_prefetch_max_ns',0)/1e9:.1f} s längster Leser). Violations: {r['crawler_violations']}")


if __name__ == "__main__":
    import sys

    which = sys.argv[1] if len(sys.argv) > 1 else "causes"
    if which == "causes":
        table_causes()
    elif which == "groups":
        table_groups()
    else:
        one_million(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else which)
