#!/usr/bin/env python3
"""M1-N22: fasst os-loadtest-Berichte (perf-001) für die A/B-Probe zusammen.

Aufruf: summarize.py <bericht.json> [...]   (Tabelle je Bericht; Verlauf je 100 Batches aus dem diag-Block)
"""
import json
import sys


def diag(m):
    d = m.get("crawler_diag", {})
    out = {}
    if isinstance(d, dict):
        for g, vals in d.items():
            if isinstance(vals, dict):
                for k, v in vals.items():
                    out[f"{g}_{k}"] = v
            else:
                out[g] = vals
    return out


def series(d, prefix, n=16, sep="_h"):
    return [d.get(f"{prefix}{sep}{i:02d}", 0) for i in range(n)]


def main():
    for path in sys.argv[1:]:
        r = json.load(open(path))
        m = r["metrics"]
        d = diag(m)
        print(f"== {path}")
        print(
            f"Mittel {m['crawler_urls_per_s']:.1f} URLs/s, {m['crawler_seconds']:.1f} s, "
            f"RSS {m['crawler_peak_rss_bytes']/1e6:.1f} MB, Stillstand {m['crawler_max_stall_s']:.2f} s, "
            f"Datei {m['crawler_file_bytes']/1e6:.0f} MB"
        )
        seg = m["crawler_segment_urls_per_s"]
        mean = m["crawler_urls_per_s"]
        print("Abschnitte:", " ".join(f"{x:.0f}" for x in seg), "|", " ".join(f"{100*x/mean:.0f}%" for x in seg))
        print("Verstöße:", m.get("crawler_violations"))
        nb = series(d, "store_batches")
        used = [i for i, n in enumerate(nb) if n]
        if not used:
            print("(keine Verlaufszähler)")
            continue
        print("Eimer (100 Batches):", used, "Batches:", [nb[i] for i in used])
        def per(name):
            v = series(d, name)
            return [v[i] / nb[i] / 1e6 if nb[i] else 0 for i in used]
        for name, label in [
            ("store_commit_ns", "Commit ms/Batch"),
            ("store_build_ns", "Aufbau ms/Batch"),
            ("store_g_urls_ins_ns", " urls einfügen"),
            ("store_g_urls_upd_ns", " urls aktualisieren"),
            ("store_g_links_ns", " links"),
            ("store_g_counters_ns", " url_counters"),
            ("store_g_queue_ns", " queue"),
            ("store_g_rest_ns", " Rest"),
            ("store_wal_cp_ns", "WAL-Rückführung ms/Batch"),
        ]:
            print(f"{label:28s}", " ".join(f"{x:8.1f}" for x in per(name)))
        cm = series(d, "store_commit_max_ns")
        print(f"{'Commit max ms':28s}", " ".join(f"{cm[i]/1e6:8.0f}" for i in used))
        print(f"{'cache_miss/Batch':28s}", " ".join(f"{series(d,'store_cache_miss')[i]/nb[i]:8.0f}" for i in used))
        print(f"{'cache_write/Batch':28s}", " ".join(f"{series(d,'store_cache_write')[i]/nb[i]:8.0f}" for i in used))
        print(f"{'wal_bytes_max MB':28s}", " ".join(f"{series(d,'store_wal_bytes_max')[i]/1e6:8.0f}" for i in used))
        tot = {k: d.get(k, 0) for k in [
            "store_build_ns", "store_commit_ns", "store_commit_max_ns", "store_wal_cp_n", "store_wal_cp_ns",
            "store_wal_cp_max_ns", "store_wal_cp_frames", "store_cache_hit", "store_cache_miss", "store_cache_write",
            "store_cache_spill", "store_link_index_build_ns", "store_checkpoints", "store_checkpoint_ns",
            "engine_w_write_ns", "engine_w_write_max_ns", "engine_q_refills", "engine_q_refill_ns",
            "engine_q_refill_wait_ns", "engine_q_refill_wait_max_ns", "engine_q_refill_read_ns",
            "engine_q_refill_read_max_ns", "engine_q_refill_rows", "engine_q_starved_n", "engine_q_starved_ns",
            "engine_q_starved_max_ns", "engine_q_activate_ns", "engine_q_activate_wait_ns",
        ]}
        print("Summen:", {k: (round(v / 1e9, 2) if k.endswith("_ns") else v) for k, v in tot.items()})
        rn = series(d, "engine_q_refill_n", sep="_r")
        ru = [i for i, n in enumerate(rn) if n]
        if ru:
            w = series(d, "engine_q_refill_wait_ns", sep="_r")
            rd = series(d, "engine_q_refill_read_ns", sep="_r")
            st = series(d, "engine_q_starved_ns", sep="_r")
            print("Refill je 10: n", [rn[i] for i in ru])
            print("  Wartezeit ms/Refill", " ".join(f"{w[i]/rn[i]/1e6:7.0f}" for i in ru))
            print("  Lesezeit ms/Refill ", " ".join(f"{rd[i]/rn[i]/1e6:7.0f}" for i in ru))
            print("  Starved s (Summe)  ", " ".join(f"{st[i]/1e9:7.2f}" for i in ru))
        print()


main()
