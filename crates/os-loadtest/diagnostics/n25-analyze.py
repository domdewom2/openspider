#!/usr/bin/env python3
"""M2-N25: wertet die JSON-Datei von apps/desktop/e2e/n25/resume-probe.spec.ts aus (Raten, CPU, Overview)."""
import json, sys

d = json.load(open(sys.argv[1]))
s = d["samples"]


def phase(label):
    return [x for x in s if x.get("label") == label and x.get("crawled") is not None]


def report(label, rows):
    if len(rows) < 2:
        print(label, "zu wenig Proben")
        return
    # nur Proben mit Phase running
    rows = [r for r in rows if r.get("phase") == "running"] or rows
    a, b = rows[0], rows[-1]
    dt = b["t"] - a["t"] or 1e-9
    print(f"{label}: {a['crawled']}->{b['crawled']} in {dt:.1f}s = {(b['crawled'] - a['crawled']) / dt:.1f} URL/s;"
          f" bridge CPU {(b['bridgeCpu'] - a['bridgeCpu']) / dt * 100:.0f} %,"
          f" browser {(b['browser'] - a['browser']) / dt * 100:.0f} %, node {(b['node'] - a['node']) / dt * 100:.0f} %")
    w = []
    for i in range(len(rows)):
        j = i
        while j < len(rows) and rows[j]["t"] - rows[i]["t"] < 10:
            j += 1
        if j < len(rows):
            w.append(f"{rows[i]['t']:.0f}s:{(rows[j]['crawled'] - rows[i]['crawled']) / (rows[j]['t'] - rows[i]['t']):.0f}")
    print("  10s-Fenster:", " ".join(w[:: max(1, len(w) // 15)]))
    print("  WAL MB/writer-CPU-s:", " ".join(f"{r['t']:.0f}:{r.get('wal', 0) / 1e6:.0f}/{r.get('writer')}" for r in rows[:: max(1, len(rows) // 15)]))
    print("  rate/threads:", " ".join(f"{r['t']:.0f}:{r.get('rate', 0):.0f}/{r.get('threads')}" for r in rows[:: max(1, len(rows) // 15)]))


print("ui", d.get("ui"), "events", d["events"])
report("vor Pause", phase("run1"))
report("Pause", phase("paused"))
report("nach Fortsetzen", phase("run2"))
report("danach", phase("after"))


def diff(a, b):
    return sorted(((k, round(b.get(k, 0) - a.get(k, 0), 2)) for k in b), key=lambda kv: -kv[1])[:10]


print("Bridge-Threads CPU-s bis Pause:", sorted(d["threadsBefore"].items(), key=lambda kv: -kv[1])[:10])
print("Bridge-Threads CPU-s Pause..Ende:", diff(d["threadsPaused"], d["threadsEnd"]))
calls = d["calls"]
by = {}
pend = [c for c in calls if c.get("pending")]
print("Am Ende offene Aufrufe:", [(c["cmd"], round((c["at"] - min(x["at"] for x in calls)) / 1000, 1)) for c in pend])
calls = [c for c in calls if "ms" in c]
for c in calls:
    k = c["cmd"]
    by.setdefault(k, []).append(c["ms"])
print("Aufrufe (Anzahl, Median ms, Max ms):")
for k, v in sorted(by.items(), key=lambda kv: -len(kv[1])):
    v2 = sorted(v)
    print(f"  {k}: {len(v)} {v2[len(v2) // 2]} {v2[-1]}")
t0 = d.get("t0") or min((x["at"] - x["ms"] for x in calls), default=0)
ev = {e["ev"]: e["t"] for e in d["events"]}
print("Overview über die Zeit (t = Antwortzeitpunkt seit Crawl-Start):")
for c in calls:
    if c["cmd"] == "sidebar.overview" and c.get("summary"):
        m = {r["key"]: r["count"] for r in c["summary"]}
        print(f"  {(c['at'] - t0) / 1000:6.1f} enc={m.get('Total URLs Encountered')} crawled={m.get('Total URLs Crawled')}"
              f" disp={m.get('Total URLs Displayed')} int={m.get('Total Internal URLs')} ext={m.get('Total External URLs')} ms={c['ms']}")
