// Vertragstest: die generierten Wrapper entsprechen dem Katalog commands.json (docs/plan/M2.md 3.6 und 3.8).
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import commands from "../commands.json";
import { createClient, setClient } from "../client";
import { MockCore } from "./mock";

type Entry = { name: string; args: string; ret: string; longRunning: boolean };
const catalog = commands as Entry[];

const modules = import.meta.glob<Record<string, Record<string, (...a: unknown[]) => Promise<unknown>>>>(
  ["../*.ts", "!../*.test.ts", "!../client.ts", "!../errors.ts", "!../events.ts"],
  { eager: true },
);

function wrappers(): Map<string, (...a: unknown[]) => Promise<unknown>> {
  const out = new Map<string, (...a: unknown[]) => Promise<unknown>>();
  for (const mod of Object.values(modules)) {
    for (const [domain, fns] of Object.entries(mod)) {
      if (typeof fns !== "object") continue;
      const dom = domain.replace(/Api$/, "");
      for (const [short, fn] of Object.entries(fns)) out.set(`${dom}.${short}`, fn);
    }
  }
  return out;
}

afterEach(() => setClient(null));

describe("IPC-Wrapper ↔ commands.json", () => {
  it("für jeden Command des Katalogs gibt es genau einen Wrapper und umgekehrt", () => {
    const w = wrappers();
    expect([...w.keys()].sort()).toEqual(catalog.map((c) => c.name).sort());
  });

  it("jeder Wrapper ruft seinen Command mit den übergebenen Argumenten auf", async () => {
    const core = new MockCore();
    for (const c of catalog) core.handlers.set(c.name, () => undefined);
    setClient(createClient(core.transport()));
    for (const [name, fn] of wrappers()) {
      const entry = catalog.find((c) => c.name === name) as Entry;
      core.calls.length = 0;
      if (entry.args === "void") await fn();
      else await fn({ probe: name });
      expect(core.calls.map((x) => x.cmd), name).toEqual([name]);
      if (entry.args !== "void") expect(core.calls[0]?.args).toEqual({ probe: name });
    }
  });

  it("die eingecheckten Wrapper sind aktuell (gen:ipc --check)", () => {
    const script = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "scripts", "gen-ipc.mjs");
    expect(() => execFileSync(process.execPath, [script, "--check"], { stdio: "pipe" })).not.toThrow();
  });
});
