#!/usr/bin/env python3
"""M1-P32: fasst die Messung M eines Schritts (Ordner mit store.json, perf-001.json, perf-003.json, *-segments.json,
log-*.txt) in eine Kurzfassung zusammen und schreibt sie als summary.json daneben. Kein Produktcode."""
import json
import re
import sys
from pathlib import Path


def load(path):
    try:
        return json.loads(Path(path).read_text())
    except (OSError, ValueError):
        return None


def load_line(log):
    """Erste `load averages`-Zeile (Beginn des Blocks) aus einer Logdatei."""
    try:
        for line in Path(log).read_text().splitlines():
            if "load average" in line:
                m = re.search(r"load averages?:\s*(.*)$", line)
                return m.group(1).strip() if m else line.strip()
    except OSError:
        pass
    return None


def main(out):
    out = Path(out)
    summary = {"dir": out.name}
    store = load(out / "store.json")
    if store:
        rel = store["segment_rel_to_mean"]
        summary["store"] = {
            "pages": store["pages"],
            "pages_per_s": round(store["pages_per_s"], 1),
            "segment_s": [round(s, 1) for s in store["segment_s"]],
            "segment_rel_to_mean_pct": [round(r * 100) for r in rel],
            "min_max_pct": [round(min(rel) * 100), round(max(rel) * 100)],
            "commit_max_s": round(store["commit_max_s"], 2),
            "checkpoint_max_s": round(store["checkpoint_max_s"], 2),
            "end_batch_s": round(store["end_batch_s"], 1),
            "finish_s": round(store["finish_s"], 1),
            "file_mib": store["file_bytes"] // (1024 * 1024),
            "peak_rss_mib": store["peak_rss_bytes"] // (1024 * 1024),
            "load_at_start": load_line(out / "log-store.txt"),
        }
    for name in ("perf-001", "perf-003"):
        rep = load(out / f"{name}.json")
        if not rep:
            continue
        m = rep["metrics"]
        entry = {
            "passed": rep.get("passed"),
            "urls_per_s": m.get("crawler_urls_per_s"),
            "seconds": m.get("crawler_seconds"),
            "peak_rss_mb": round(m["crawler_peak_rss_bytes"] / 1e6, 1) if m.get("crawler_peak_rss_bytes") else None,
            "max_stall_s": m.get("crawler_max_stall_s"),
            "file_bytes": m.get("crawler_file_bytes"),
            "violations": m.get("crawler_violations"),
            "load_at_start": load_line(out / f"log-{name}.txt"),
        }
        diag = m.get("crawler_diag") or {}
        for key in (
            "store_commit_max_ns",
            "store_commit_ns",
            "store_checkpoint_max_ns",
            "store_checkpoint_ns",
            "store_checkpoints",
            "store_bg_checkpoints",
            "store_bg_checkpoint_max_ns",
            "store_wait_checkpoint_ns",
            "store_link_index_build_ns",
            "engine_w_write_max_ns",
            "engine_send_ns",
        ):
            if key in diag:
                entry[key] = diag[key]
        seg = load(out / f"{name}-segments.json")
        if seg:
            entry["segments"] = {
                "complete": seg["segments_complete"],
                "mean_pages_per_s": seg["mean_pages_per_s"],
                "rel_pct": [round(s["rel_to_mean"] * 100) for s in seg["segments"]],
                "min_max_pct": [round(seg["min_rel"] * 100), round(seg["max_rel"] * 100)],
            }
        summary[name] = entry
    (out / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps(summary, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main(sys.argv[1])
