import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";

function descendants(rows, rootPid) {
  const ids = new Set([rootPid]);
  for (let i = 0; i < rows.length; i++) {
    let added = false;
    for (const p of rows)
      if (ids.has(p.parent) && !ids.has(p.pid)) {
        ids.add(p.pid);
        added = true;
      }
    if (!added) break;
  }
  return rows.filter((p) => ids.has(p.pid));
}
export function rendererProcesses(rows, rootPid) {
  return descendants(rows, rootPid).filter((p) =>
    /--type=renderer|WebKitWebProcess|com\.apple\.WebKit\.WebContent|msedgewebview2\.exe/i.test(
      p.command,
    ),
  );
}

// Enumerate short process names first. Command lines from other workspaces can be
// arbitrarily large; never request them when measuring this worker's process tree.
export function rendererRss(rootPid = process.pid) {
  if (!Number.isInteger(rootPid) || rootPid < 1)
    throw new Error("Invalid RSS root PID");
  let rows;
  if (process.platform === "win32") {
    const script = `$all=@(Get-CimInstance Win32_Process); $ids=[System.Collections.Generic.HashSet[int]]::new(); [void]$ids.Add(${rootPid}); do { $changed=$false; foreach($p in $all) { if($ids.Contains([int]$p.ParentProcessId) -and $ids.Add([int]$p.ProcessId)) { $changed=$true } } } while($changed); ConvertTo-Json -Compress -InputObject @($all | Where-Object { $ids.Contains([int]$_.ProcessId) } | Select-Object ProcessId,ParentProcessId,WorkingSetSize,KernelModeTime,UserModeTime,CommandLine)`;
    rows = JSON.parse(
      execFileSync("powershell.exe", ["-NoProfile", "-Command", script], {
        encoding: "utf8",
        maxBuffer: 16 * 1024 * 1024,
      }),
    ).map((p) => ({
      pid: p.ProcessId,
      parent: p.ParentProcessId,
      bytes: Number(p.WorkingSetSize),
      cpuMs: (Number(p.KernelModeTime) + Number(p.UserModeTime)) / 10000,
      command: p.CommandLine ?? "",
    }));
  } else {
    rows = execFileSync("ps", ["-axo", "pid=,ppid=,rss=,time=,comm="], {
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
    })
      .trim()
      .split("\n")
      .map((line) => {
        const [, pid, parent, rss, cpu, command] =
          line.match(/^\s*(\d+)\s+(\d+)\s+(\d+)\s+([\d:.-]+)\s+(.*)$/) ?? [];
        return {
          pid: Number(pid),
          parent: Number(parent),
          bytes: Number(rss) * 1024,
          cpuMs: cpu
            ?.split(":")
            .reverse()
            .reduce((n, v, i) => n + Number(v) * 60 ** i * 1000, 0),
          command: command ?? "",
        };
      });
    rows = descendants(rows, rootPid);
    if (process.platform === "linux") {
      for (const p of rows) {
        try {
          p.command = readFileSync(`/proc/${p.pid}/cmdline`, "utf8").replaceAll(
            "\0",
            " ",
          );
        } catch (error) {
          if (error.code !== "ENOENT" && error.code !== "ESRCH") throw error;
          p.command = "";
        }
      }
    } else if (rows.length) {
      const commands = execFileSync(
        "ps",
        ["-p", rows.map((p) => p.pid).join(","), "-o", "pid=,args="],
        { encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
      );
      const byPid = new Map(
        commands
          .trim()
          .split("\n")
          .map((line) => {
            const [, pid, command] = line.match(/^\s*(\d+)\s+(.*)$/) ?? [];
            return [Number(pid), command ?? ""];
          }),
      );
      for (const p of rows) p.command = byPid.get(p.pid) ?? "";
    }
  }
  let processes = rendererProcesses(rows, rootPid);
  if (process.platform === "linux") {
    const hz = Number(
      execFileSync("getconf", ["CLK_TCK"], { encoding: "utf8" }),
    );
    processes = processes.filter((p) => {
      try {
        const stat = readFileSync(`/proc/${p.pid}/stat`, "utf8")
          .split(") ")[1]
          .split(" ");
        p.cpuMs = ((Number(stat[11]) + Number(stat[12])) / hz) * 1000;
        return true;
      } catch (error) {
        if (error.code === "ENOENT" || error.code === "ESRCH") return false;
        throw error;
      }
    });
  }
  if (!processes.length)
    throw new Error(
      "No attributable renderer process: RSS measurement unavailable",
    );
  return {
    scope: processes.some((p) => /msedgewebview2\.exe/i.test(p.command))
      ? "webview2-process-tree"
      : "renderer-processes",
    cpuMs: processes.every((p) => Number.isFinite(p.cpuMs))
      ? processes.reduce((n, p) => n + p.cpuMs, 0)
      : null,
    bytes: processes.reduce((n, p) => n + p.bytes, 0),
    processes: processes.map(({ pid, bytes }) => ({ pid, bytes })),
  };
}
