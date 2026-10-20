import { spawn, type ChildProcess } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { bridgeBinary } from "../support/bridge";

export async function stop(child: ChildProcess): Promise<void> {
  if (!child.pid || child.exitCode !== null || child.signalCode !== null) return;
  await new Promise<void>(resolve => {
    const timer = setTimeout(() => child.kill("SIGKILL"), 5_000);
    child.once("exit", () => { clearTimeout(timer); resolve(); }); child.kill();
  });
}
export async function firstLine(child: ChildProcess): Promise<string> {
  return new Promise((resolve, reject) => {
    let buffer = "";
    const timer = setTimeout(() => { child.kill(); reject(new Error("Real service startup timeout")); }, 20_000);
    const fail = (error: Error) => { clearTimeout(timer); reject(error); };
    child.once("error", fail);
    child.once("exit", code => fail(new Error(`Real service exited: ${code}`)));
    child.stdout!.on("data", chunk => {
      buffer += String(chunk);
      if (buffer.includes("\n")) { clearTimeout(timer); resolve(buffer.split("\n")[0]!); }
    });
  });
}
export async function startRealBridge(options: { home?: string } = {}) {
  const home = options.home ?? await mkdtemp(join(tmpdir(), "os-u25-real-"));
  const cleanup = async () => { if (!options.home) await rm(home, { force: true, recursive: true }); };
  const child = spawn(bridgeBinary(), ["--home", home], { env: { ...process.env, OPENSPIDER_HOME: home, OPENSPIDER_SECRET_STORE: "memory" }, stdio: ["ignore", "pipe", "inherit"] });
  try {
    const { port, token } = JSON.parse(await firstLine(child)) as { port: number; token: string };
    return { home, url: `ws://127.0.0.1:${port}/?token=${token}`, stop: async () => { await stop(child); await cleanup(); } };
  } catch (error) { await stop(child); await cleanup(); throw error; }
}
export async function outBasicSite() {
  const binary = join(process.env.CARGO_TARGET_DIR ?? resolve("../../target"), "debug", process.platform === "win32" ? "os-testsite.exe" : "os-testsite");
  const child = spawn(binary, ["out-basic-site", "0"], { stdio: ["ignore", "pipe", "inherit"] });
  try {
    const line = await firstLine(child);
    const url = line.match(/https?:\/\/\S+/)?.[0];
    if (!url) throw new Error(`Invalid os-testsite startup: ${line}`);
    return { url: new URL("/", url).href, stop: () => stop(child) };
  } catch (error) { await stop(child); throw error; }
}
