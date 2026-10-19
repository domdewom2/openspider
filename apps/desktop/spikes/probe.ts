// Measurement-only entry, loaded by WebDriver from Vite. Never part of the app bundle.
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { openSettingsWindow } from "../src/app/windows";
import { eventBus } from "../src/ipc/events";
import { registerAction } from "../src/ui/actions";

const ticks: { at: number; seq: number; phase: string | undefined }[] = [];
const visibility: { at: number; value: string }[] = [];
const actions: string[] = [];
const keys: { key: string; trusted: boolean; target: string }[] = [];
eventBus.on("tick", (tick) => ticks.push({ at: performance.now(), seq: tick.seq, phase: tick.crawl?.phase }));
document.addEventListener("visibilitychange", () => visibility.push({ at: performance.now(), value: document.visibilityState }));
document.addEventListener("keydown", (e) => keys.push({ key: e.key, trusted: e.isTrusted, target: (e.target as HTMLElement).id }), true);
document.addEventListener("os:action", (e) => actions.push((e as CustomEvent<{ id: string }>).detail.id));

export async function snapshot() {
  const win = getCurrentWindow();
  return {
    at: performance.now(), frontend: document.visibilityState, focus: document.hasFocus(),
    shell: { visible: await win.isVisible(), minimized: await win.isMinimized() },
    // These are native window inputs, not a claim to read the private os-app visibility state.
    transport: window.__osRuntime?.transport, menu: window.__osMenu,
    ticks: [...ticks], visibility: [...visibility], actions: [...actions], keys: [...keys],
  };
}

export async function activate() {
  const win = getCurrentWindow();
  await win.show();
  await win.setFocus();
  return snapshot();
}

export function installKeyboardFixture() {
  document.getElementById("s02-fixture")?.remove();
  const root = document.createElement("section");
  root.id = "s02-fixture";
  root.style.cssText = "position:fixed;inset:80px 40px;background:white;color:black;z-index:9999;padding:24px";
  root.innerHTML = '<label>Spike input <input id="s02-input" value="fixture alpha"></label><label>Paste target <textarea id="s02-paste"></textarea></label><div id="s02-grid" role="grid" tabindex="0" data-region="table"><div role="row"><span role="gridcell">fixture alpha</span></div><div role="row"><span role="gridcell">fixture beta</span></div></div>';
  document.body.append(root);
  // Observe the real dispatcher with a fixture action endpoint, not a production grid implementation.
  registerAction("table.copy", () => {});
  registerAction("table.selectAll", () => {});
}

export const settings = openSettingsWindow;
export const minimize = () => getCurrentWindow().minimize();
export async function restore() {
  const win = getCurrentWindow();
  if (await win.isMinimized()) await win.unminimize();
  return activate();
}
export const close = () => getCurrentWindow().close();
// The shell reserves Tauri's built-in app namespace; its own app domain is os-app.
export const requestQuit = () => invoke("plugin:os-app|requestQuit");
export const start = (url: string) => invoke("plugin:crawl|start", { source: { kind: "spider", url }, scope: "domain", profile: null });

export function keyboardSnapshot() {
  const input = document.querySelector<HTMLInputElement>("#s02-input")!;
  const paste = document.querySelector<HTMLTextAreaElement>("#s02-paste")!;
  return { value: input.value, selection: [input.selectionStart, input.selectionEnd], paste: paste.value, actions: [...actions], keys: [...keys] };
}
