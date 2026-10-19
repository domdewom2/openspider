import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { bridgeBinary, type Bridge } from "../support/bridge";

export async function startGridBridge(): Promise<Bridge> {
  const home = await mkdtemp(join(tmpdir(), "os-grid-"));
  const child = spawn(bridgeBinary(), ["--home", home, "--stub-views"], { stdio: ["ignore", "pipe", "inherit"] });
  try {
    const info = await new Promise<{ port: number; token: string }>((resolve, reject) => {
      let buffer = "";
      const timer = setTimeout(() => { child.kill(); reject(new Error("stub-views bridge timed out")); }, 20_000);
      child.on("error", error => { clearTimeout(timer); reject(error); });
      child.on("exit", code => { clearTimeout(timer); reject(new Error(`stub-views bridge exited: ${code}`)); });
      child.stdout.on("data", chunk => {
        buffer += String(chunk);
        if (buffer.includes("\n")) { clearTimeout(timer); try { resolve(JSON.parse(buffer.split("\n")[0])); } catch (error) { reject(error); } }
      });
    });
    return { url: `ws://127.0.0.1:${info.port}/?token=${info.token}`, stop: async () => {
      if (child.exitCode === null) { const exited = new Promise<void>(resolve => child.once("exit", () => resolve())); child.kill(); await exited; }
      await rm(home, { recursive: true, force: true });
    } };
  } catch (error) { child.kill(); await rm(home, { recursive: true, force: true }); throw error; }
}
