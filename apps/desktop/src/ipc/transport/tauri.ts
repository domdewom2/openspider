// Transport über Tauri: Commands als interne Plugins `plugin:<domäne>|<command>` (M2-ui-concept 4.1), Ereignisse
// über einen `Channel<UiEvent>` je Fenster.
import { Channel, invoke as tauriInvoke } from "@tauri-apps/api/core";
import { toIpcError } from "../errors";
import type { Transport, UiEvent } from "./types";

export function isTauri(win: object = globalThis): boolean {
  return "__TAURI_INTERNALS__" in win;
}

/** `view.rows` → `plugin:view|rows` (Namen wie in commands.json). */
export function tauriCommandName(cmd: string): string {
  const dot = cmd.indexOf(".");
  if (dot < 0) return cmd;
  return `plugin:${cmd.slice(0, dot)}|${cmd.slice(dot + 1)}`;
}

export function createTauriTransport(): Transport {
  return {
    name: "tauri",
    async invoke<T>(cmd: string, args: unknown): Promise<T> {
      try {
        return await tauriInvoke<T>(tauriCommandName(cmd), (args ?? {}) as Record<string, unknown>);
      } catch (e) {
        throw toIpcError(e);
      }
    },
    subscribe(window: string, onEvent: (e: UiEvent) => void): () => void {
      let open = true;
      const channel = new Channel<UiEvent>();
      channel.onmessage = (ev) => {
        if (open) onEvent(ev);
      };
      void tauriInvoke(tauriCommandName("events.subscribe"), { window, channel }).catch(() => {
        // Ohne Kanal kommen keine Ticks; der Fehler wird beim nächsten Command sichtbar.
      });
      return () => {
        open = false;
      };
    },
  };
}

/** Additive shell helper (M2-U03, K2-61): native notification plugin, no app command bypass. */
export async function sendDesktopNotification(title: string, body: string): Promise<void> {
  const granted = await tauriInvoke<boolean | null>("plugin:notification|is_permission_granted");
  if (granted === false) return;
  if (granted === null && await tauriInvoke<string>("plugin:notification|request_permission") !== "granted") return;
  await tauriInvoke("plugin:notification|notify", { options: { title, body } });
}

/** Additive shell helper (M2-U03, K2-61): copy diagnostic text through the installed clipboard plugin. */
export async function copyDesktopText(text: string): Promise<void> {
  await tauriInvoke("plugin:clipboard-manager|write_text", { text });
}
