// Additive process attribution only: preserves the unchanged U31 process set.
import { readFileSync, readlinkSync } from "node:fs";

export function processProvenance(sample: { processes: { pid: number; bytes: number }[] }) {
  if (process.platform !== "linux") return { platform: process.platform, supported: false };
  return sample.processes.map(renderer => {
    const chain: { pid: number; executable: string | null }[] = [];
    const seen = new Set<number>();
    let pid = renderer.pid;
    while (pid > 0 && !seen.has(pid) && chain.length < 64) {
      seen.add(pid);
      try {
        let executable: string | null = null;
        try { executable = readlinkSync(`/proc/${pid}/exe`); } catch { /* process can exit between samples */ }
        chain.push({ pid, executable });
        if (pid === process.pid) break;
        const status = readFileSync(`/proc/${pid}/status`, "utf8");
        pid = Number(/^PPid:\s*(\d+)/m.exec(status)?.[1] ?? 0);
      } catch { break; }
    }
    return { ...renderer, underCore: chain.some(p => p.executable?.endsWith("/os-app-bridge")), chain };
  });
}

/** Kernel mapping accounting only; does not alter the U31 RSS process set. */
export function summarizeMappings(smaps: string) {
  const groups = new Map<string, { name: string; rssBytes: number; pssBytes: number; privateDirtyBytes: number }>();
  let current: ReturnType<typeof groups.get>;
  for (const line of smaps.split("\n")) {
    const header = /^[\da-f]+-[\da-f]+\s+[rwxps-]{4}\s+[\da-f]+\s+[\da-f]+:[\da-f]+\s+\d+\s*(.*)$/i.exec(line);
    if (header) {
      const name = header[1] || "[anonymous]";
      current = groups.get(name);
      if (!current) { current = { name, rssBytes: 0, pssBytes: 0, privateDirtyBytes: 0 }; groups.set(name, current); }
      continue;
    }
    const value = /^(Rss|Pss|Private_Dirty):\s+(\d+) kB$/.exec(line);
    if (!current || !value) continue;
    const key = value[1] === "Rss" ? "rssBytes" : value[1] === "Pss" ? "pssBytes" : "privateDirtyBytes";
    current[key] += Number(value[2]) * 1024;
  }
  const mappings = [...groups.values()].sort((a, b) => b.rssBytes - a.rssBytes);
  return {
    rssBytes: mappings.reduce((sum, m) => sum + m.rssBytes, 0),
    pssBytes: mappings.reduce((sum, m) => sum + m.pssBytes, 0),
    privateDirtyBytes: mappings.reduce((sum, m) => sum + m.privateDirtyBytes, 0),
    mappings,
  };
}

export function processMemoryMappings(sample: { processes: { pid: number; bytes: number }[] }) {
  if (process.platform !== "linux") return { platform: process.platform, supported: false };
  return sample.processes.map(({ pid }) => {
    try { return { pid, ...summarizeMappings(readFileSync(`/proc/${pid}/smaps`, "utf8")) }; }
    catch (error) { return { pid, unavailable: String((error as NodeJS.ErrnoException).code ?? error) }; }
  });
}
