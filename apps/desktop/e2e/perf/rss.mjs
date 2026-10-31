import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
// Descendants only: other jobs on the build server must never enter this measurement.
export function rendererRss(rootPid = process.pid) {
  let rows;
  if (process.platform === "win32") {
    rows = JSON.parse(
      execFileSync(
        "powershell.exe",
        [
          "-NoProfile",
          "-Command",
          "ConvertTo-Json -Compress -InputObject @(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,WorkingSetSize,KernelModeTime,UserModeTime,CommandLine)",
        ],
        { encoding: "utf8" },
      ),
    ).map((p) => ({
      pid: p.ProcessId,
      parent: p.ParentProcessId,
      bytes: Number(p.WorkingSetSize),
      cpuMs: (Number(p.KernelModeTime) + Number(p.UserModeTime)) / 10000,
      command: p.CommandLine ?? "",
    }));
  } else {
    rows = execFileSync("ps", ["-axo", "pid=,ppid=,rss=,time=,args="], {
      encoding: "utf8",
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
  }
  const processes = rendererProcesses(rows, rootPid);
  if (!processes.length)
    throw new Error(
      "No attributable renderer process: RSS measurement unavailable",
    );
  if (process.platform === "linux") {
    const hz = Number(
      execFileSync("getconf", ["CLK_TCK"], { encoding: "utf8" }),
    );
    for (const p of processes) {
      const stat = readFileSync(`/proc/${p.pid}/stat`, "utf8")
        .split(") ")[1]
        .split(" ");
      p.cpuMs = ((Number(stat[11]) + Number(stat[12])) / hz) * 1000;
    }
  }
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

export function rendererProcesses(rows, rootPid) {
  const descendants = new Set([rootPid]);
  for (let i = 0; i < rows.length; i++) {
    let added = false;
    for (const p of rows)
      if (descendants.has(p.parent) && !descendants.has(p.pid)) {
        descendants.add(p.pid);
        added = true;
      }
    if (!added) break;
  }
  return rows.filter(
    (p) =>
      descendants.has(p.pid) &&
      /--type=renderer|WebKitWebProcess|com\.apple\.WebKit\.WebContent|msedgewebview2\.exe/i.test(
        p.command,
      ),
  );
}
