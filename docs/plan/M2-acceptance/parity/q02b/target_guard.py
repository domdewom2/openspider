"""Nur eigene Buildartefakte: RAM-Auslagerung vor Tests, Aufräumen nach Tests.

Aufruf während des Cargo-Laufs; keine Änderung oder Symbolbereinigung von
Testprogrammen. /tmp ist auf diesem Buildserver ein ausführbares tmpfs.
"""
import json
import os
import pathlib
import re
import shutil
import sys
import tempfile
import time

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[4]
rams = [pathlib.Path(tempfile.mkdtemp(prefix="openspider-q02b-guard-", dir=directory))
        for directory in [None, *sys.argv[1:]]]
for ram in rams:
    (ram / "deps").mkdir()
    for binary in ["openspider", "os-parity", "os-testsite", "os-loadtest", "os-app-bridge", "os_app_manifest"]:
        (ram / binary).symlink_to(ROOT / "target/debug" / binary)
    for directory in ["examples", "build", ".fingerprint"]:
        (ram / directory).symlink_to(ROOT / "target/debug" / directory)
(HERE / "ram-guard-paths.json").write_text(json.dumps([str(ram) for ram in rams]) + "\n")
removed = set()
seen_cargo = False
while True:
    log = HERE / "workspace.log"
    text = log.read_text(errors="replace") if log.exists() else ""
    free = shutil.disk_usage(ROOT).free
    incremental = ROOT / "target/debug/incremental"
    if free < 25 * 1024**3 and incremental.exists():
        shutil.rmtree(incremental)
    running = re.findall(r"Running .*?\((target/debug/deps/[^\s)]+)\)", text)
    # Cargo compiles all regular targets before starting the first test.
    # Offload only during that phase: a running program retains its exact ELF.
    if not running and free < 32 * 1024**3:
        for source in (ROOT / "target/debug/deps").iterdir():
            try:
                stat = source.stat()
                if (source.is_symlink() or source.suffix or not source.is_file()
                        or not stat.st_mode & 0o111 or stat.st_nlink != 1
                        or stat.st_mtime > time.time() - 10):
                    continue
                available = next(int(line.split()[1]) * 1024 for line in
                                 pathlib.Path("/proc/meminfo").read_text().splitlines()
                                 if line.startswith("MemAvailable:"))
                ram = next((ram for ram in rams if shutil.disk_usage(ram).free >= 7 * 1024**3), None)
                if ram is None or available < 14 * 1024**3:
                    break
                target = ram / "deps" / source.name
                shutil.copy2(source, target)
                link = source.with_name(source.name + ".q02b-ram-link")
                link.symlink_to(target)
                link.replace(source)
                if shutil.disk_usage(ROOT).free >= 35 * 1024**3:
                    break
            except FileNotFoundError:
                continue
    # Only targets whose successor has already started have finished.
    for name in running[:-1]:
        if name in removed:
            continue
        source = ROOT / name
        if source.is_symlink():
            target = source.resolve()
            if any(target.is_relative_to(ram) for ram in rams):
                target.unlink(missing_ok=True)
        source.unlink(missing_ok=True)
        removed.add(name)
    own_cargo = False
    for proc in pathlib.Path("/proc").iterdir():
        try:
            args = (proc / "cmdline").read_bytes().split(b"\0")
            if (args and pathlib.Path(args[0].decode()).name == "cargo"
                    and b"test" in args and (proc / "cwd").resolve() == ROOT):
                own_cargo = True
        except (OSError, UnicodeDecodeError):
            continue
    seen_cargo |= own_cargo
    print(json.dumps(dict(completed_test_programs_removed=len(removed),
                          free_bytes=shutil.disk_usage(ROOT).free,
                          ram_free_bytes=[shutil.disk_usage(ram).free for ram in rams])), flush=True)
    if seen_cargo and not own_cargo:
        break
    time.sleep(5)
