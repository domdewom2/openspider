import { afterEach, describe, expect, it, vi } from "vitest";
import { createBridgeTransport, toUiEvent, type WebSocketLike } from "./bridge";
import { createClient, detectTransport, setClient, VersionGuard } from "../client";
import { classifyError, describeError, IpcError, isSilentError, localError, toCmdError } from "../errors";
import { bindVisibility, connectWindowEvents, createEventBus } from "../events";
import { MockCore, mergePatch } from "./mock";
import { tauriCommandName } from "./tauri";
import { settings } from "../settings";
import { view } from "../view";
import { exportApi } from "../export";
import type { Tick } from "../gen/Tick";
import type { UiEvent } from "../gen/UiEvent";
import cmdErrorFixture from "../__fixtures__/CmdError.json";
import tickFixture from "../__fixtures__/UiEvent.tick.json";

afterEach(() => setClient(null));

class FakeSocket implements WebSocketLike {
  readyState = 0;
  sent: Record<string, unknown>[] = [];
  onopen: WebSocketLike["onopen"] = null;
  onmessage: WebSocketLike["onmessage"] = null;
  onclose: WebSocketLike["onclose"] = null;
  onerror: WebSocketLike["onerror"] = null;
  send(data: string) {
    this.sent.push(JSON.parse(data));
  }
  close() {
    this.readyState = 3;
    this.onclose?.({});
  }
  open() {
    this.readyState = 1;
    this.onopen?.({});
  }
  push(msg: unknown) {
    this.onmessage?.({ data: JSON.stringify(msg) });
  }
}

describe("Fehlerumsetzung (CmdError)", () => {
  it("übernimmt das Kernformat unverändert (Fixture CmdError.json)", () => {
    const e = toCmdError(cmdErrorFixture);
    expect(e.code).toBe("InvalidInput");
    expect(e.field).toBe("scope.include[0]");
    expect(e.args).toMatchObject({ position: 4 });
    expect(classifyError(e)).toBe("validation");
  });

  it("Stale und Cancelled sind still, Internal ist fatal, Io ist ein Banner", () => {
    expect(isSilentError(localError("Stale", "x"))).toBe(true);
    expect(isSilentError(localError("Cancelled", "x"))).toBe(true);
    expect(classifyError(localError("Internal", "x"))).toBe("fatal");
    expect(classifyError(localError("Io", "x"))).toBe("banner");
    expect(isSilentError(localError("Io", "x"))).toBe(false);
  });

  it("macht aus Exceptions, Strings und Abbrüchen ein CmdError", () => {
    expect(toCmdError(new Error("boom")).detail).toBe("boom");
    expect(toCmdError("text").code).toBe("Internal");
    expect(toCmdError(new DOMException("x", "AbortError")).code).toBe("Cancelled");
  });

  it("describeError übersetzt messageKey mit Argumenten und hält die technische Ursache getrennt", () => {
    const t = (k: string, a?: Record<string, unknown>) => `${k}:${JSON.stringify(a)}`;
    const e = { ...localError("Io", "err-export-dir"), args: { path: "/x" }, detail: "EACCES" };
    expect(describeError(t, e)).toEqual({ text: 'err-export-dir:{"path":"/x","code":"Io"}', detail: "EACCES" });
    const unknown = describeError(t, e, () => false);
    expect(unknown.text.startsWith("err-code-io")).toBe(true);
  });
});

describe("Client", () => {
  it("setzt Fehler des Transports in IpcError um", async () => {
    const core = new MockCore();
    const client = createClient(core.transport());
    await expect(client.invoke("gibt.es.nicht")).rejects.toMatchObject({ name: "IpcError", code: "Unsupported" });
  });

  it("bricht vor dem Senden ab, wenn das Signal schon abgebrochen ist", async () => {
    const core = new MockCore();
    const client = createClient(core.transport());
    const ac = new AbortController();
    ac.abort();
    await expect(client.invoke("settings.get", {}, ac.signal)).rejects.toMatchObject({ code: "Cancelled" });
    expect(core.calls).toHaveLength(0);
  });

  it("AbortSignal löst bei lang laufenden Commands op.cancel aus", async () => {
    const core = new MockCore();
    core.handlers.set("export.view", () => ({ opId: "op-7", labelKey: "op-export", cancellable: true }));
    const client = createClient(core.transport());
    setClient(client);
    const ac = new AbortController();
    const handle = await exportApi.view({} as never, ac.signal);
    expect(handle.opId).toBe("op-7");
    expect(core.calls.some((c) => c.cmd === "op.cancel")).toBe(false);
    ac.abort();
    await vi.waitFor(() => expect(core.calls.find((c) => c.cmd === "op.cancel")?.args).toEqual({ opId: "op-7" }));
  });

  // spec: UI-142
  it("verwirft Antworten mit älterer View-Version als Stale", async () => {
    const core = new MockCore();
    const versions = [5, 7, 6];
    core.handlers.set("view.rows", () => ({ viewId: "v1", version: versions.shift(), offset: 0, rowKeys: [], columns: [], cells: [] }));
    setClient(createClient(core.transport()));
    await view.rows({} as never);
    await view.rows({} as never);
    await expect(view.rows({} as never)).rejects.toMatchObject({ code: "Stale" });
  });

  it("VersionGuard unterscheidet Ansichten und kann vergessen", () => {
    const g = new VersionGuard();
    g.accept({ viewId: "a", version: 3 });
    g.accept({ viewId: "b", version: 1 });
    expect(() => g.accept({ viewId: "a", version: 2 })).toThrow(IpcError);
    g.forget("a");
    expect(g.accept({ viewId: "a", version: 1 })).toBeTruthy();
    expect(g.accept("kein Objekt")).toBe("kein Objekt");
  });

  it("wählt den Transport: Tauri vor Bridge vor Mock; die Query-Auswahl nur im Testbuild", () => {
    expect(detectTransport({ win: { __TAURI_INTERNALS__: {} } }).choice).toBe("tauri");
    expect(detectTransport({ win: {}, bridgeEnv: "ws://127.0.0.1:1/?token=a" })).toEqual({ choice: "bridge", bridgeUrl: "ws://127.0.0.1:1/?token=a" });
    expect(detectTransport({ win: {}, search: "?bridge=ws%3A%2F%2Fx", testBuild: true })).toEqual({ choice: "bridge", bridgeUrl: "ws://x" });
    expect(detectTransport({ win: {}, search: "?bridge=ws%3A%2F%2Fx", testBuild: false }).choice).toBe("mock");
    expect(detectTransport({ win: {}, search: "?transport=mock", bridgeEnv: "ws://y", testBuild: true }).choice).toBe("mock");
  });

  it("bildet Tauri-Commandnamen auf Plugin-Aufrufe ab", () => {
    expect(tauriCommandName("view.rows")).toBe("plugin:view|rows");
    expect(tauriCommandName("crawl.validateStartUrl")).toBe("plugin:crawl|validateStartUrl");
  });
});

describe("Bridge-Transport (Protokoll docs/plan/M2.md 3.6)", () => {
  it("sendet Anfragen, ordnet Antworten über die id zu und setzt err in IpcError um", async () => {
    const sock = new FakeSocket();
    const t = createBridgeTransport({ url: "ws://127.0.0.1:1/?token=ab", createSocket: () => sock });
    const a = t.invoke("view.rows", { n: 1 });
    const b = t.invoke("view.rows", { n: 2 });
    expect(sock.sent).toHaveLength(0); // erst nach onopen
    sock.open();
    expect(sock.sent).toEqual([
      { id: 1, cmd: "view.rows", args: { n: 1 } },
      { id: 2, cmd: "view.rows", args: { n: 2 } },
    ]);
    sock.push({ id: 2, err: cmdErrorFixture });
    sock.push({ id: 1, ok: { fertig: true } });
    await expect(a).resolves.toEqual({ fertig: true });
    await expect(b).rejects.toMatchObject({ code: "InvalidInput" });
  });

  it("abonniert je Fenster einmal und verteilt Ereignisse als UiEvent", () => {
    const sock = new FakeSocket();
    sock.readyState = 1;
    const t = createBridgeTransport({ url: "ws://x", createSocket: () => sock });
    const got: UiEvent[] = [];
    const off = t.subscribe("main", (e) => got.push(e));
    t.subscribe("main", () => undefined);
    expect(sock.sent.filter((m) => m.cmd === "__subscribe")).toEqual([{ cmd: "__subscribe", window: "main" }]);
    sock.push({ event: "tick", window: "main", data: (tickFixture as { tick: Tick }).tick });
    sock.push({ event: "settingsChanged", window: "main", data: { keys: ["appearance.theme"] } });
    sock.push({ event: "tick", window: "settings", data: {} });
    expect(got.map((e) => e.kind)).toEqual(["tick", "settingsChanged"]);
    off();
    sock.push({ event: "notice", window: "main", data: { level: "info", code: "x", args: {} } });
    expect(got).toHaveLength(2);
  });

  it("lehnt offene Anfragen ab, wenn die Verbindung endet", async () => {
    const sock = new FakeSocket();
    const t = createBridgeTransport({ url: "ws://x", createSocket: () => sock });
    const p = t.invoke("app.info", {});
    sock.open();
    sock.close();
    await expect(p).rejects.toMatchObject({ code: "Io" });
    await expect(t.invoke("app.info", {})).rejects.toMatchObject({ code: "Io" });
  });

  it("toUiEvent kennt tick, settingsChanged und notice", () => {
    expect(toUiEvent("settingsChanged", ["a"])).toEqual({ kind: "settingsChanged", keys: ["a"] });
    expect(toUiEvent("unbekannt", {})).toBeNull();
  });
});

describe("Mock-Kern", () => {
  it("settings.set wendet einen JSON-Merge-Patch an, meldet geänderte Schlüssel und Neustartpflicht", async () => {
    const core = new MockCore();
    const events: UiEvent[] = [];
    core.addSub("settings", (e) => events.push(e));
    setClient(createClient(core.transport("main")));
    const next = await settings.set({ patch: { appearance: { theme: "dark" }, storage: { crawlsDir: "/x" } } });
    expect(next.appearance.theme).toBe("dark");
    expect(next.restartRequired).toEqual(["storage.crawlsDir"]);
    expect(events).toEqual([{ kind: "settingsChanged", keys: ["appearance.theme", "storage.crawlsDir"] }]);
  });

  it("mergePatch: null löscht, verschachtelte Objekte werden gemischt", () => {
    expect(mergePatch({ a: { b: 1, c: 2 }, d: 1 }, { a: { b: null }, d: 5 })).toEqual({ a: { c: 2 }, d: 5 });
  });
});

describe("Event-Bus (ein Kanal je Fenster)", () => {
  it("verteilt Tick, settingsChanged und Notice an die Abonnenten", async () => {
    const core = new MockCore();
    const bus = createEventBus();
    const ticks: Tick[] = [];
    const keys: string[][] = [];
    bus.on("tick", (t) => ticks.push(t));
    bus.on("settingsChanged", (k) => keys.push(k));
    const off = connectWindowEvents(createClient(core.transport("main")), "main", bus);
    expect(core.subscribers("main")).toBe(1);
    core.emit("main", tickFixture as UiEvent);
    core.emit("main", { kind: "settingsChanged", keys: ["general.language"] });
    core.emit("settings", { kind: "settingsChanged", keys: ["anderes.fenster"] });
    expect(ticks).toHaveLength(1);
    expect(keys).toEqual([["general.language"]]);
    off();
    expect(core.subscribers("main")).toBe(0);
  });

  it("meldet die Sichtbarkeit bei visibilitychange (UI-143)", async () => {
    const core = new MockCore();
    setClient(createClient(core.transport("main")));
    const off = bindVisibility("main");
    document.dispatchEvent(new Event("visibilitychange"));
    await vi.waitFor(() => expect(core.calls.find((c) => c.cmd === "events.setVisibility")?.args).toMatchObject({ window: "main" }));
    off();
  });
});
