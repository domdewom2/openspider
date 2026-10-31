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
