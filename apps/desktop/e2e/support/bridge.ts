// Start und Ende der E2E-Bridge `os-app-bridge` (docs/plan/M2.md 3.6): gibt auf stdout eine Zeile {"port":n,"token":"hex"} aus.
import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

export interface Bridge {
  /** `ws://127.0.0.1:<port>/?token=<hex>` */
  url: string;
  stderr?(): string;
  stop(): Promise<void>;
}

/** Pfad der Bridge-Binärdatei: `OS_APP_BRIDGE_BIN`, sonst `<target>/debug/os-app-bridge`. */
export function bridgeBinary(): string {
  if (process.env.OS_APP_BRIDGE_BIN) return process.env.OS_APP_BRIDGE_BIN;
  const target = process.env.CARGO_TARGET_DIR ?? resolve(import.meta.dirname, "..", "..", "..", "..", "target");
  return join(target, "debug", process.platform === "win32" ? "os-app-bridge.exe" : "os-app-bridge");
}

export function bridgeAvailable(): boolean {
  return existsSync(bridgeBinary());
}

/** Startet die Bridge mit leerem Home und (optional) Fixture; `null`, wenn sie nicht startet (z. B. Stub ohne Laufzeit). */
export async function startBridge(opts: { fixture?: string; timeoutMs?: number } = {}): Promise<Bridge | null> {
  if (!bridgeAvailable()) return null;
  const home = mkdtempSync(join(tmpdir(), "os-bridge-"));
  const args = ["--home", home, ...(opts.fixture ? ["--fixture", opts.fixture] : [])];
  const child: ChildProcess = spawn(bridgeBinary(), args, { stdio: ["ignore", "pipe", "pipe"] });
  let stderr = "";
  child.stderr?.on("data", (d: Buffer) => { stderr = (stderr + d.toString()).slice(-100_000); process.stderr.write(d); });
  // Windows hält Dateien des beendeten Prozesses kurz gesperrt (EBUSY): erneut versuchen, Aufräumen ist nur Best Effort.
  const cleanup = () => {
    try {
      rmSync(home, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
    } catch {
      /* Temp-Verzeichnis bleibt liegen */
    }
  };

  const line = await new Promise<string | null>((resolveLine) => {
    let buf = "";
    const timer = setTimeout(() => resolveLine(null), opts.timeoutMs ?? 20_000);
    child.stdout?.on("data", (d: Buffer) => {
      buf += d.toString();
      const nl = buf.indexOf("\n");
      if (nl >= 0) {
        clearTimeout(timer);
        resolveLine(buf.slice(0, nl));
      }
    });
    child.on("exit", () => {
      clearTimeout(timer);
      resolveLine(null);
    });
  });

  let info: { port: number; token: string } | null = null;
  try {
    info = line ? (JSON.parse(line) as { port: number; token: string }) : null;
  } catch {
    info = null;
  }
  if (!info) {
    child.kill();
    cleanup();
    return null;
  }
  return {
    url: `ws://127.0.0.1:${info.port}/?token=${info.token}`,
    stderr: () => stderr,
    async stop() {
      child.kill();
      await new Promise((r) => setTimeout(r, 50));
      cleanup();
    },
  };
}
