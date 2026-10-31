import { spawn } from "node:child_process";
import { bridgeBinary } from "../support/bridge";

/** Restart the real app process while retaining only its on-disk home. */
export async function persistentBridge(home: string) {
  const child = spawn(bridgeBinary(), ["--home", home], { stdio: ["ignore", "pipe", "inherit"] });
  const info = await new Promise<{ port: number; token: string }>((resolve, reject) => {
    let output = "";
    const timer = setTimeout(() => reject(new Error("Bridge startup timeout")), 20_000);
    child.stdout.on("data", (data: Buffer) => {
      output += data.toString();
      const end = output.indexOf("\n");
      if (end >= 0) { clearTimeout(timer); try { resolve(JSON.parse(output.slice(0, end))); } catch (error) { reject(error); } }
    });
    child.once("error", error => { clearTimeout(timer); reject(error); });
    child.once("exit", () => { clearTimeout(timer); reject(new Error("Bridge exited during startup")); });
  });
  return { url: `ws://127.0.0.1:${info.port}/?token=${info.token}`, async stop() {
    if (child.exitCode !== null || child.signalCode !== null) return;
    await new Promise<void>(resolve => { child.once("exit", () => resolve()); child.kill(); });
  } };
}
