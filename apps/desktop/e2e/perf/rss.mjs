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
// macOS Playwright WebKit starts WebContent as XPC services through launchd
// (parent PID 1), so they are not descendants of the browser. Attribute them via
// the browser bundle launched by this process tree and its start time. A live
// browser of the same bundle outside this tree that could have started a
// candidate makes attribution ambiguous: fail instead of guessing.
const playwrightWebKit = /^(.+)\/Playwright\.app\/Contents\/MacOS\/Playwright$/;
export function webkitXpcRenderers(rows, rootPid) {
  const own = new Set(descendants(rows, rootPid).map((p) => p.pid));
  const browsers = rows.flatMap((p) => {
    const bundle = playwrightWebKit.exec(p.command)?.[1];
    return bundle ? [{ ...p, bundle, own: own.has(p.pid) }] : [];
  });
  const result = [];
  for (const bundle of new Set(
    browsers.filter((b) => b.own).map((b) => b.bundle),
  )) {
    const service = `${bundle}/com.apple.WebKit.WebContent.xpc/Contents/MacOS/com.apple.WebKit.WebContent`;
    const sameBundle = browsers.filter((b) => b.bundle === bundle);
    for (const p of rows) {
      if (
        p.parent !== 1 ||
        (p.command !== service && p.command !== `${service}.Development`)
      )
        continue;
      if (!Number.isFinite(p.started)) continue;
      const possible = sameBundle.filter((b) => b.started <= p.started);
      if (!possible.some((b) => b.own)) continue;
      if (possible.some((b) => !b.own))
        throw new Error(
          "Ambiguous WebKit renderer attribution: RSS measurement unavailable",
        );
      result.push(p);
    }
  }
  return result;
}
const elapsedSeconds = (etime) => {
  const [days, clock] = etime.includes("-") ? etime.split("-") : ["0", etime];
  return (
    Number(days) * 86400 +
    clock
      .split(":")
      .reverse()
      .reduce((n, v, i) => n + Number(v) * 60 ** i, 0)
  );
};
export function rendererProcesses(rows, rootPid) {
  return descendants(rows, rootPid).filter((p) =>
    /--type=renderer|WebKitWebProcess|WPEWebProcess|com\.apple\.WebKit\.WebContent|msedgewebview2\.exe/i.test(
      p.command,
    ),
  );
}

// Enumerate short process names first. Command lines from other workspaces can be
// arbitrarily large; never request them when measuring this worker's process tree.
export function rendererRss(rootPid = process.pid) {
  if (!Number.isInteger(rootPid) || rootPid < 1)
    throw new Error("Invalid RSS root PID");
  let rows, all;
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
    const darwin = process.platform === "darwin";
    all = execFileSync(
      "ps",
      [
        "-axo",
        darwin
          ? "pid=,ppid=,rss=,time=,etime=,comm="
          : "pid=,ppid=,rss=,time=,comm=",
      ],
      {
        encoding: "utf8",
        maxBuffer: 16 * 1024 * 1024,
      },
    )
      .trim()
      .split("\n")
      .map((line) => {
        const [, pid, parent, rss, cpu, ...rest] =
          line.match(
            darwin
              ? /^\s*(\d+)\s+(\d+)\s+(\d+)\s+([\d:.-]+)\s+([\d:-]+)\s+(.*)$/
              : /^\s*(\d+)\s+(\d+)\s+(\d+)\s+([\d:.-]+)\s+(.*)$/,
          ) ?? [];
        const command = rest.at(-1);
        const etime = darwin ? rest[0] : undefined;
        return {
          pid: Number(pid),
          parent: Number(parent),
          bytes: Number(rss) * 1024,
          cpuMs: cpu
            ?.split(":")
            .reverse()
            .reduce((n, v, i) => n + Number(v) * 60 ** i * 1000, 0),
          // Relative start in seconds within this snapshot (macOS only).
          ...(etime ? { started: -elapsedSeconds(etime) } : {}),
          command: command ?? "",
        };
      });
    // Copies: command lines below must not replace executable paths in `all`.
    rows = descendants(all, rootPid).map((p) => ({ ...p }));
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
  let scope = "renderer-processes";
  if (!processes.length && process.platform === "darwin") {
    processes = webkitXpcRenderers(all, rootPid);
    scope = "webkit-xpc-renderers";
  }
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
      : scope,
    cpuMs: processes.every((p) => Number.isFinite(p.cpuMs))
      ? processes.reduce((n, p) => n + p.cpuMs, 0)
      : null,
    bytes: processes.reduce((n, p) => n + p.bytes, 0),
    processes: processes.map(({ pid, bytes }) => ({ pid, bytes })),
  };
}
