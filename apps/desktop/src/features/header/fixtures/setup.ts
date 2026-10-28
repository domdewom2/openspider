import { createClient, setClient } from "../../../ipc/client";
import type { Tick } from "../../../ipc/gen/Tick";
import { MockCore } from "../../../ipc/transport/mock";
import { useSession } from "../../../state/session";
import { useHeader } from "../model";
import fixture from "./tick-running.json";

export const tick = fixture as Tick;
export function setupHeader() {
  useSession.setState({ seq: -1, crawl: null, analysis: null, analysisResult: null, analysisStarting: false, mcp: null, throttled: {}, crawlDelays: {}, notices: [], views: {}, ops: {} });
  useHeader.setState({ mode: "spider", url: "", scope: "domain" });
  const core = new MockCore();
  core.handlers.set("crawl.validateStartUrl", (args) => ({ normalized: (args as { input: string }).input }));
  core.handlers.set("crawl.start", () => ({ crawlId: "crawl-test", file: "saved.ospider" }));
  for (const cmd of ["crawl.pause", "crawl.resume", "crawl.clear"]) core.handlers.set(cmd, () => undefined);
  core.handlers.set("crawl.parseList", (args) => ({ valid: (args as { text: string }).text.split("\n").length, invalid: 0, duplicates: 0, sample: [] }));
  core.handlers.set("config.get", () => ({ schemaVersion: 2, generator: "fixture", config: { speed: { threads: 5 }, keep: true } }));
  core.handlers.set("config.set", () => ({ applied: [], requiresRecrawl: [] }));
  setClient(createClient(core.transport()));
  return core;
}
