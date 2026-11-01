import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { bridgeBinary, type Bridge } from "../support/bridge";
export async function startPerfBridge(): Promise<Bridge> {
  const home = await mkdtemp(join(tmpdir(), "os-u31-"));
  const child = spawn(bridgeBinary(), ["--home", home], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  let errors = "";
  child.stderr.on("data", (chunk) => {
    errors = (errors + String(chunk)).slice(-100_000);
  });
  // A failed Playwright worker must not leave a busy Rust bridge in the next benchmark.
  const onExit = () => {
    child.kill("SIGKILL");
  };
  process.on("exit", onExit);
  try {
    const info = await new Promise<{ port: number; token: string }>(
      (resolve, reject) => {
        const timer = setTimeout(
          () => reject(new Error(`Bridge timeout: ${errors}`)),
          30_000,
        );
        let buffer = "";
        child.on("error", (error) => {
          clearTimeout(timer);
          reject(error);
        });
        child.on("exit", (code) => {
          clearTimeout(timer);
          reject(new Error(`Bridge exit ${code}: ${errors}`));
        });
        child.stdout.on("data", (chunk) => {
          buffer += String(chunk);
          if (buffer.includes("\n")) {
            clearTimeout(timer);
            try {
              resolve(JSON.parse(buffer.split("\n")[0]));
            } catch (error) {
              reject(error);
            }
          }
        });
      },
    );
    return {
      url: `ws://127.0.0.1:${info.port}/?token=${info.token}`,
      stderr: () => errors,
      async stop() {
        process.off("exit", onExit);
        if (child.exitCode === null && child.signalCode === null) {
          const exited = new Promise<void>((resolve) =>
            child.once("exit", () => resolve()),
          );
          child.kill("SIGKILL");
          await exited;
        }
        await rm(home, {
          recursive: true,
          force: true,
          maxRetries: 10,
          retryDelay: 100,
        });
      },
    };
  } catch (error) {
    onExit();
    process.off("exit", onExit);
    await rm(home, { recursive: true, force: true });
    throw error;
  }
}
