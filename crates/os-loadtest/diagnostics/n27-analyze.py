#!/usr/bin/env python3
"""M2-N27: wertet die JSON-Datei der N25-Probe (n25-resume-probe.spec.ts) nach UI-018 aus.

Je Abschnitt (Lauf bis Pause, Pause, Lauf nach Fortsetzen, nach dem Ende) die CPU der Bridge aus /proc,
dazu die Verzögerung der Overview: für jede Antwort von `sidebar.overview` die Zeit seit dem Zeitpunkt, an
dem der Crawl den gezeigten Wert „Total URLs Crawled“ erreicht hatte (lineare Interpolation der Proben im
Sekundenabstand), und nach Pause bzw. Ende die Zeit bis zur ersten Antwort mit dem pausierten bzw. finalen
Wert. Aufruf: n27-analyze.py <probe.json> [weitere …]
"""
import json
import statistics
import sys


def analyze(path):
    d = json.load(open(path))
    samples = [s for s in d["samples"] if s.get("bridgeCpu") is not None]
    ev = {e["ev"]: e["t"] for e in d["events"]}
    t0 = d["t0"]
    end = next((s["t"] for s in samples if s["label"] == "after"), samples[-1]["t"])
    sections = [
        ("Lauf bis Pause", samples[0]["t"], ev["paused"]),
        ("Pause", ev["paused"], ev["resumed"]),
        ("Lauf nach Fortsetzen", ev["resumed"], end),
        ("nach dem Ende (15 s)", end, samples[-1]["t"]),
    ]

    def cpu_at(t):
        before = [s for s in samples if s["t"] <= t]
        after = [s for s in samples if s["t"] >= t]
        a = before[-1] if before else samples[0]
        b = after[0] if after else samples[-1]
        if b["t"] == a["t"]:
            return a["bridgeCpu"], a["t"]
        return a["bridgeCpu"] + (b["bridgeCpu"] - a["bridgeCpu"]) * (t - a["t"]) / (b["t"] - a["t"]), t

    print(f"== {path}")
    for name, a, b in sections:
        (ca, ta), (cb, tb) = cpu_at(a), cpu_at(b)
        if tb - ta > 0.5:
            print(f"  Bridge-CPU {name}: {(cb - ca) / (tb - ta) * 100:.0f} % ({tb - ta:.0f} s)")

    crawled = [(s["t"], s["crawled"]) for s in samples if s.get("crawled") is not None]

    def reached(value):
        """Zeitpunkt, an dem der Crawl `value` gecrawlte URLs erreicht hatte."""
        prev = None
        for t, c in crawled:
            if c >= value:
                if prev is None or c == prev[1]:
                    return t
                return prev[0] + (t - prev[0]) * (value - prev[1]) / (c - prev[1])
            prev = (t, c)
        return None

    answers = []
    for c in d["calls"]:
        if c["cmd"] == "sidebar.overview" and c.get("summary"):
            m = {r["key"]: r["count"] for r in c["summary"]}
            answers.append(((c["at"] - t0) / 1000, m.get("Total URLs Crawled"), c["ms"]))
    running = [(t, v, ms) for t, v, ms in answers if t < ev["paused"] or ev["resumed"] <= t < end]
    lags = []
    for t, v, ms in running:
        r = reached(v)
        if r is not None:
            lags.append(max(0.0, t - r))
    if lags:
        print(f"  Overview während des Laufs: {len(lags)} Antworten, Verzögerung Median {statistics.median(lags):.1f} s,"
              f" Max {max(lags):.1f} s")
    # Alter des angezeigten Stands je Probe während des Laufs: letzte Antwort vor der Probe, deren Wert der Crawl
    # zum Zeitpunkt reached(v) erreicht hatte (das sieht der Nutzer).
    ages = []
    for t, c in crawled:
        if not (t < ev["paused"] or ev["resumed"] <= t < end):
            continue
        shown = [v for at, v, ms in answers if at <= t]
        r = reached(shown[-1]) if shown else None
        if r is not None:
            ages.append(max(0.0, t - r))
    if ages:
        ages.sort()
        print(f"  Alter des angezeigten Overview-Stands während des Laufs: Median {statistics.median(ages):.1f} s,"
              f" 90 % {ages[int(len(ages) * 0.9) - 1]:.1f} s, Max {ages[-1]:.1f} s ({len(ages)} Proben)")
    # Nach der Pause ist die erste Antwort auf eine Anfrage exakt, die nach der Pause begann (Stand ohne laufenden
    # Crawl); nach dem Ende die erste Antwort mit dem finalen Wert (mindestens so viele wie die letzte Probe).
    final = crawled[-1][1] if crawled else None
    checks = (
        ("Pause", ev["paused"], lambda t, v, ms: t - ms / 1000 >= ev["paused"]),
        ("Ende", end, lambda t, v, ms: t >= end and final is not None and v is not None and v >= final),
    )
    for name, start, exact in checks:
        hit = next(((t, v) for t, v, ms in answers if exact(t, v, ms)), None)
        print(f"  Overview exakt nach {name}: "
              + (f"{hit[0] - start:.1f} s danach (gecrawlt {hit[1]})" if hit is not None else "nicht erreicht"))
    print("  Antworten (t, gecrawlt, ms):", " ".join(f"{t:.1f}/{v}/{ms}" for t, v, ms in answers))
    by = {}
    for c in d["calls"]:
        if "ms" in c and c["cmd"] in ("sidebar.overview", "sidebar.issues", "catalog.filterCounts"):
            by.setdefault(c["cmd"], []).append(c["ms"])
    for k, v in sorted(by.items()):
        v = sorted(v)
        print(f"  {k}: {len(v)} Aufrufe, Median {v[len(v) // 2]} ms, Max {v[-1]} ms")
    pending = [c["cmd"] for c in d["calls"] if c.get("pending")]
    if pending:
        print("  am Ende offen:", pending)


for p in sys.argv[1:]:
    analyze(p)
