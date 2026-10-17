// Fenster öffnen und Beenden: Einstellungsfenster (eigenes Fenster `settings`, nicht modal), Beenden über den
// gemeinsamen Schließpfad (F-11).
import { getT } from "../i18n";
import { isTauri } from "../ipc/transport/tauri";

export const SETTINGS_WINDOW = "settings";
export const OPEN_SETTING_EVENT = "os:open-setting";

/** Öffnet das Einstellungsfenster bzw. holt es nach vorn; `anchor` wählt Abschnitt und Steuerelement (UI-016). */
export async function openSettingsWindow(anchor?: string): Promise<void> {
  const hash = anchor ? `#${encodeURIComponent(anchor)}` : "";
  if (isTauri()) {
    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const existing = await WebviewWindow.getByLabel(SETTINGS_WINDOW);
    if (existing) {
      await existing.show();
      await existing.setFocus();
      if (anchor) {
        const { emitTo } = await import("@tauri-apps/api/event");
        await emitTo(SETTINGS_WINDOW, OPEN_SETTING_EVENT, anchor);
      }
      return;
    }
    new WebviewWindow(SETTINGS_WINDOW, {
      url: `settings.html${hash}`,
      title: getT()("app-settings-title"),
      width: 880,
      height: 620,
      minWidth: 720,
      minHeight: 480,
    });
    return;
  }
  // Browser (E2E-A, Harness): benanntes Fenster – ein zweiter Aufruf holt es nach vorn statt ein neues zu öffnen.
  const url = `settings.html${typeof location !== "undefined" ? location.search : ""}${hash}`;
  window.open(url, SETTINGS_WINDOW, "popup,width=900,height=640")?.focus();
}

/** Beenden/Schließen läuft über `CloseRequested` des Hauptfensters (Rückfrage bei laufendem Crawl, F-11). */
export async function quitApp(): Promise<void> {
  if (isTauri()) {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
    return;
  }
  window.close();
}
