"""Tests für run.sh und compare.sh mit einer Attrappe als openspider (kein Netz, keine echte Binärdatei)."""

import json
import os
import shutil
import subprocess
import tempfile
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
SHADOW = os.path.dirname(HERE)

FAKE = r'''#!/usr/bin/env python3
import json, os, sys, time
sys.path.insert(0, os.environ["FAKE_COMPARE_DIR"])
import compare
a = sys.argv[1:]
def val(flag):
    return a[a.index(flag) + 1]
log = os.environ["FAKE_LOG"]
with open(log, "a") as fh:
    fh.write("start %s\n" % val("--crawl"))
    fh.write("args %s\n" % json.dumps(a))
out = val("--output-folder")
os.makedirs(out, exist_ok=True)
names = [n.replace("\\,", ",") for n in val("--export-tabs").split(",")]
for n in names:
    with open(os.path.join(out, compare.export_file_name(n)), "w") as fh:
        fh.write("﻿Address\nhttps://example.test/\n")
with open(os.path.join(out, "crawl.ospider"), "w") as fh:
    fh.write("fake")
print(json.dumps({"phase": "crawl", "crawled": 4, "elapsed_s": 2, "rate": 2.0}), file=sys.stderr)
time.sleep(0.3)
with open(log, "a") as fh:
    fh.write("end %s\n" % val("--crawl"))
sys.exit(int(os.environ.get("FAKE_EXIT", "0")))
'''


class ShadowScriptTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.base = os.path.join(self.tmp, "shadow")
        for d in ("bin", "etc", "var"):
            os.makedirs(os.path.join(self.base, d))
        for f in ("run.sh", "compare.sh", "compare.py"):
            shutil.copy(os.path.join(SHADOW, f), os.path.join(self.base, "bin", f))
        for f in ("shadow.osconfig", "shadow.env", "rule-map.toml", "autonomo-source.toml"):
            shutil.copy(os.path.join(SHADOW, f), os.path.join(self.base, "etc", f))
        fake = os.path.join(self.base, "bin", "openspider")
        with open(fake, "w") as fh:
            fh.write(FAKE)
        os.chmod(fake, 0o755)
        self.log = os.path.join(self.tmp, "fake.log")
        self.env = {
            **os.environ,
            "SHADOW_BASE": self.base,
            "SHADOW_SITES": "one.test two.test",
            "SHADOW_KEEP": "2",
            "SHADOW_MIN_FREE_GB": "0",
            "SHADOW_SAMPLE_S": "0.1",
            "FAKE_LOG": self.log,
            "FAKE_COMPARE_DIR": os.path.join(self.base, "bin"),
            "OPENSPIDER_HOME": os.path.join(self.base, "var", "home"),
        }

    def run_script(self, name, **extra):
        return subprocess.run(["bash", os.path.join(self.base, "bin", name)], env={**self.env, **extra},
                              capture_output=True, text=True, timeout=60, check=False)

    def test_sites_run_one_after_the_other_with_the_agreed_arguments(self):
        proc = self.run_script("run.sh")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        with open(self.log) as fh:
            lines = [ln.split(" ", 1)[0] + " " + ln.split(" ", 1)[1].strip() for ln in fh if ln.split(" ", 1)[0] != "args"]
        self.assertEqual(lines, ["start https://one.test/", "end https://one.test/",
                                 "start https://two.test/", "end https://two.test/"])
        with open(self.log) as fh:
            args = json.loads([ln for ln in fh if ln.startswith("args")][0][5:])
        for flag in ("--headless", "--save-crawl", "--no-store", "--json-progress"):
            self.assertIn(flag, args)
        self.assertEqual(args[args.index("--config") + 1], os.path.join(self.base, "etc", "shadow.osconfig"))
        self.assertEqual(args[args.index("--export-format") + 1], "csv")
        self.assertIn("Internal:All", args[args.index("--export-tabs") + 1].split(","))
        with open(os.path.join(self.base, "var", "latest-run.json")) as fh:
            latest = json.load(fh)
        self.assertEqual(latest["sites"], ["one.test", "two.test"])
        run_json = os.path.join(self.base, "var", "exports", "one.test", latest["stamp"], "run.json")
        with open(run_json) as fh:
            meta = json.load(fh)
        self.assertEqual((meta["exit_code"], meta["site"]), (0, "one.test"))
        self.assertTrue(os.path.isfile(os.path.join(self.base, "var", "crawls", "one.test", latest["stamp"] + ".ospider")))
        self.assertFalse(os.path.exists(os.path.join(self.base, "var", "exports", "one.test", latest["stamp"], "crawl.ospider")))

    def test_only_the_newest_runs_are_kept(self):
        for _ in range(3):
            self.assertEqual(self.run_script("run.sh").returncode, 0)
            time.sleep(1.1)
        for kind, suffix in (("exports", ""), ("crawls", ".ospider"), ("crawls", ".progress.log")):
            names = [n for n in os.listdir(os.path.join(self.base, "var", kind, "one.test")) if n.endswith(suffix)]
            self.assertEqual(len(names), 2, (kind, suffix, names))

    def test_too_little_free_space_aborts_before_writing(self):
        proc = self.run_script("run.sh", SHADOW_MIN_FREE_GB="999999999")
        self.assertEqual(proc.returncode, 75)
        self.assertIn("GB frei", proc.stderr)
        self.assertFalse(os.path.exists(os.path.join(self.base, "var", "exports")))
        self.assertFalse(os.path.exists(self.log))

    def test_a_failed_crawl_is_recorded_and_stops_the_comparison(self):
        proc = self.run_script("run.sh", FAKE_EXIT="3")
        self.assertEqual(proc.returncode, 0)
        with open(os.path.join(self.base, "var", "latest-run.json")) as fh:
            stamp = json.load(fh)["stamp"]
        with open(os.path.join(self.base, "var", "exports", "two.test", stamp, "run.json")) as fh:
            self.assertEqual(json.load(fh)["exit_code"], 3)
        cmp = self.run_script("compare.sh", SHADOW_AUTONOMO_SOURCE="file", SHADOW_AUTONOMO_FILE="/nicht/vorhanden")
        self.assertNotEqual(cmp.returncode, 0)
        self.assertIn("kein Vergleich", cmp.stderr)

    def test_comparison_after_the_crawl_writes_reports_per_site(self):
        self.assertEqual(self.run_script("run.sh").returncode, 0)
        with open(os.path.join(self.base, "var", "latest-run.json")) as fh:
            stamp = json.load(fh)["stamp"]
        with open(os.path.join(self.base, "var", "exports", "one.test", stamp, "run.json")) as fh:
            finished = json.load(fh)["finished_at"]
        auto = os.path.join(self.tmp, "autonomo.json")
        with open(auto, "w") as fh:
            json.dump({"schema": 1, "source": "file", "crawl": {"id": "x", "finished_at": finished},
                       "findings": {"irgendeine_regel": ["https://example.test/"]}}, fh)
        proc = self.run_script("compare.sh", SHADOW_AUTONOMO_SOURCE="file", SHADOW_AUTONOMO_FILE=auto)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        reports = os.path.join(self.base, "var", "reports")
        day = os.listdir(reports)[0]
        names = sorted(os.listdir(os.path.join(reports, day)))
        self.assertEqual(names, ["autonomo-one.test.json", "autonomo-two.test.json", "one.test.json", "one.test.md",
                                 "two.test.json", "two.test.md"])
        with open(os.path.join(reports, day, "one.test.json")) as fh:
            result = json.load(fh)
        self.assertEqual(result["autonomo_unmapped"], {"irgendeine_regel": 1})
        self.assertEqual(result["metrics"]["urls_crawled"], 4)
        # Alle exportierten Filter, auch die mit Platzhaltern im Namen, wurden gefunden (nichts "Export fehlt").
        self.assertGreaterEqual(len(result["openspider_unmapped"]), 60)
        self.assertNotIn(None, result["openspider_unmapped"].values())
        self.assertEqual(result["unreadable_exports"], {})


class DeployGuardTests(unittest.TestCase):
    """deploy.sh mit Attrappen für gh, ssh und shasum im PATH: nichts wird geladen oder übertragen."""

    GH = r'''#!/usr/bin/env bash
case "$2" in
  list) echo 4711 ;;
  view) echo '{"x": 1}' ;;
  download) d=""; while [ $# -gt 0 ]; do [ "$1" = -D ] && d="$2"; shift; done; mkdir -p "$d"; echo bin >"$d/openspider"; echo "x  openspider" >"$d/openspider.sha256" ;;
esac
exit 0
'''
    SSH = r'''#!/usr/bin/env bash
echo "$*" >>"$FAKE_SSH_LOG"
case "$*" in
  *is-active*) echo "${FAKE_STATE:-inactive}" ;;
esac
cat >/dev/null 2>&1 || true
exit 0
'''

    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, self.tmp, True)
        bindir = os.path.join(self.tmp, "fakebin")
        os.makedirs(bindir)
        for name, body in (("gh", self.GH), ("ssh", self.SSH), ("shasum", "#!/usr/bin/env bash\nexit 0\n")):
            path = os.path.join(bindir, name)
            with open(path, "w") as fh:
                fh.write(body)
            os.chmod(path, 0o755)
        self.log = os.path.join(self.tmp, "ssh.log")
        self.env = {**os.environ, "PATH": bindir + os.pathsep + os.environ["PATH"], "FAKE_SSH_LOG": self.log}

    def deploy(self, state):
        return subprocess.run(["bash", os.path.join(SHADOW, "deploy.sh"), "--out", os.path.join(self.tmp, "out")],
                              env={**self.env, "FAKE_STATE": state}, capture_output=True, text=True, timeout=60,
                              check=False, cwd=self.tmp, stdin=subprocess.DEVNULL)

    def test_a_running_service_stops_deploy_before_any_change(self):
        for state in ("active", "activating"):
            proc = self.deploy(state)
            self.assertNotEqual(proc.returncode, 0, state)
            self.assertIn("läuft", proc.stderr)
            with open(self.log) as fh:
                calls = fh.read()
            self.assertIn("is-active", calls)
            self.assertNotIn("useradd", calls)
            self.assertNotIn("systemctl link", calls)
            self.assertFalse(os.path.exists(os.path.join(self.tmp, "out", "vorher.txt")))
            os.remove(self.log)

    def test_an_idle_service_lets_deploy_continue(self):
        proc = self.deploy("inactive")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        with open(self.log) as fh:
            calls = fh.read()
        self.assertIn("systemctl link", calls)
        self.assertTrue(os.path.exists(os.path.join(self.tmp, "out", "vorher.txt")))

    def test_binaries_are_replaced_atomically(self):
        with open(os.path.join(SHADOW, "deploy.sh"), encoding="utf-8") as fh:
            text = fh.read()
        self.assertIn('mv -f "$BASE/bin/.$n.new" "$BASE/bin/$n"', text)
        self.assertNotIn('cp -a "$TMP/bin/." "$BASE/bin/"', text)


class StaticFileTests(unittest.TestCase):
    def read(self, *parts):
        with open(os.path.join(SHADOW, *parts), encoding="utf-8") as fh:
            return fh.read()

    def test_unit_has_the_agreed_limits_and_hardening(self):
        unit = self.read("systemd", "openspider-shadow.service")
        for line in ("Type=oneshot", "User=openspider-shadow", "Nice=15", "IOSchedulingClass=idle", "CPUQuota=200%",
                     "MemoryMax=2G", "RuntimeMaxSec=6h", "NoNewPrivileges=true", "ProtectSystem=strict",
                     "ReadWritePaths=/opt/openspider-shadow/var", "PrivateTmp=true", "ProtectHome=true",
                     "ExecStart=/opt/openspider-shadow/bin/run.sh", "ExecStartPost=+/opt/openspider-shadow/bin/compare.sh"):
            self.assertIn(line, unit.splitlines())

    def test_timer_is_weekly_at_night_and_persistent(self):
        timer = self.read("systemd", "openspider-shadow.timer").splitlines()
        for line in ("OnCalendar=Sun *-*-* 02:30:00 UTC", "RandomizedDelaySec=15min", "Persistent=true"):
            self.assertIn(line, timer)

    def test_osconfig_is_gentle_and_polite(self):
        import tomllib
        cfg = tomllib.loads(self.read("shadow.osconfig"))
        self.assertEqual(cfg["speed"], {"threads": 2, "max_urls_per_sec": 2.0})
        self.assertEqual(cfg["user_agent"]["http"], "OpenSpider-Shadow")
        self.assertEqual(cfg["user_agent"]["robots_token"], "OpenSpider-Shadow")
        self.assertEqual(cfg["robots"]["mode"], "respect")
        self.assertEqual(cfg["rendering"]["mode"], "text")
        self.assertEqual(cfg["limits"]["max_urls"], 20000)

    def test_scripts_never_write_to_autonomo(self):
        for name in ("compare.py", "compare.sh", "run.sh"):
            text = self.read(name)
            for verb in ("POST", "PUT", "PATCH", "DELETE"):
                if name == "compare.py":
                    self.assertNotIn(f'method="{verb}"', text)
                else:
                    self.assertNotIn(f"-X {verb}", text)


if __name__ == "__main__":
    unittest.main()
