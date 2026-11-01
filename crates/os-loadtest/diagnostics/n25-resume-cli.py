#!/usr/bin/env python3
"""M2-N25: Rate vor der Pause und nach dem Fortsetzen, nur Engine (CLI, ohne Oberfläche).

Aufruf (Repo-Wurzel, Release-Build von os-cli und os-testsite):
    python3 crates/os-loadtest/diagnostics/n25-resume-cli.py [--pages 6000] [--delay 50] [--pause-at 3000]

Startet `os-testsite synthetic:n=<pages>,links={a.links},seed=7,delay_ms=<delay>`, crawlt mit `openspider --crawl`
(`--progress json`), sendet bei `--pause-at` gecrawlten URLs SIGINT (Pause, CRAWL-126), setzt mit
`--load-crawl <ID> --resume` fort und gibt je Phase Rate (URL/s) und CPU-Zeit des Crawl-Prozesses aus.
Nur Loopback.
"""
import argparse, json, os, re, signal, subprocess, sys, tempfile, threading, time

ap = argparse.ArgumentParser()
ap.add_argument("--pages", type=int, default=6000)
ap.add_argument("--delay", type=int, default=50)
ap.add_argument("--slow-share", type=float, default=1.0)
ap.add_argument("--pause-at", type=int, default=3000)
ap.add_argument("--threads", type=int, default=5)
ap.add_argument("--bin", default="target/release")
ap.add_argument("--links", type=int, default=30)
ap.add_argument("--set", action="append", default=[])
a = ap.parse_args()

TICK = os.sysconf("SC_CLK_TCK")


def cpu_s(pid):
    try:
        with open(f"/proc/{pid}/stat") as f:
            parts = f.read().rsplit(")", 1)[1].split()
        return (int(parts[11]) + int(parts[12])) / TICK
    except OSError:
        return None


def threads(pid):
    out = {}
    try:
        for t in os.listdir(f"/proc/{pid}/task"):
            try:
                with open(f"/proc/{pid}/task/{t}/stat") as f:
                    raw = f.read()
                name = raw[raw.index("(") + 1: raw.rindex(")")]
                parts = raw.rsplit(")", 1)[1].split()
                out.setdefault(name, 0.0)
                out[name] += (int(parts[11]) + int(parts[12])) / TICK
            except OSError:
                pass
    except OSError:
        pass
    return out


spec = f"synthetic:n={a.pages},links={a.links},seed=7,delay_ms={a.delay},slow_share={a.slow_share}"
site = subprocess.Popen([f"{a.bin}/os-testsite", spec], stdout=subprocess.PIPE, text=True)
line = site.stdout.readline()
base = re.search(r"-> (http://\S+)", line).group(1)
home = tempfile.mkdtemp(prefix="n25-cli-")
env = dict(os.environ, OPENSPIDER_HOME=home)
sets = [f"speed.threads={a.threads}", *a.set]


def run(args, pause_at=None):
    cmd = [f"{a.bin}/openspider", *args, "--progress", "json"]
    for s in sets:
        cmd += ["--set", s]
    p = subprocess.Popen(cmd, stderr=subprocess.PIPE, stdout=subprocess.DEVNULL, text=True, env=env)
    samples = []  # (wall, crawled, cpu)
    t0 = time.monotonic()
    sent = False
    last_threads = {}

    def reader():
        nonlocal sent
        for raw in p.stderr:
            raw = raw.strip()
            if not raw.startswith("{"):
                continue
            try:
                j = json.loads(raw)
            except ValueError:
                continue
            if j.get("phase") != "crawl":
                continue
            samples.append((time.monotonic() - t0, j.get("crawled", 0), cpu_s(p.pid), j.get("queued"), j.get("rate")))
            if pause_at and not sent and j.get("crawled", 0) >= pause_at:
                sent = True
                last_threads.update(threads(p.pid))
                p.send_signal(signal.SIGINT)

    th = threading.Thread(target=reader, daemon=True)
    th.start()
    while p.poll() is None:
        snap = threads(p.pid)
        if snap:
            last_threads.clear()
            last_threads.update(snap)
        time.sleep(0.5)
    th.join(5)
    return p.returncode, samples, dict(last_threads)


def summary(name, samples):
    if len(samples) < 2:
        print(name, "zu wenige Proben", samples)
        return
    w0, c0, cpu0, *_ = samples[0]
    w1, c1, cpu1, *_ = samples[-1]
    rate = (c1 - c0) / max(w1 - w0, 1e-9)
    cpu = ((cpu1 or 0) - (cpu0 or 0)) / max(w1 - w0, 1e-9) * 100
    print(f"{name}: {c0}->{c1} URLs in {w1 - w0:.1f} s = {rate:.1f} URL/s, CPU {cpu:.0f} %")
    # 10-s-Fenster
    windows = []
    i = 0
    for j in range(len(samples)):
        while samples[j][0] - samples[i][0] > 10:
            i += 1
        if samples[j][0] - samples[i][0] >= 9.5:
            windows.append((samples[j][0], (samples[j][1] - samples[i][1]) / (samples[j][0] - samples[i][0])))
    print("  10s-Fenster:", " ".join(f"{t:.0f}s:{r:.0f}" for t, r in windows[:: max(1, len(windows) // 12)]))


code1, s1, th1 = run(["--crawl", f"{base}/"], pause_at=a.pause_at)
print("Lauf 1 Exit", code1)
summary("vor Pause", s1)
listing = subprocess.run([f"{a.bin}/openspider", "--list-crawls", "--json"], capture_output=True, text=True, env=env)
crawls = json.loads(listing.stdout)
cid = crawls[0].get("id") or crawls[0].get("crawl_id") or crawls[0].get("ID")
code2, s2, th2 = run(["--load-crawl", str(cid), "--resume"])
print("Lauf 2 Exit", code2)
summary("nach Fortsetzen", s2)
print("Threads (CPU-s) Lauf 1:", sorted(th1.items(), key=lambda kv: -kv[1])[:8])
print("Threads (CPU-s) Lauf 2:", sorted(th2.items(), key=lambda kv: -kv[1])[:8])
site.send_signal(signal.SIGINT)
site.wait(5)
subprocess.run(["rm", "-rf", home])
