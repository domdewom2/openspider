// Fensterweiter Zustand des Konfigurationsdialogs: offen/geschlossen und das Sprungziel (UI-016: `openConfig({ anchor })`).
import { create } from "zustand";

export interface ConfigDialogState {
  open: boolean;
  /** Feldpfad oder Seitenpräfix, z. B. `rendering.mode`; das Ziel-Steuerelement erhält den Fokus. */
  anchor: string | null;
  /** Zeigt beim Öffnen die Liste der Abweichungen von den Werkseinstellungen (OUT-224, Aktion `config.diff`). */
  diff: boolean;
  /** Zählt jedes Öffnen bzw. Anspringen, damit auch derselbe Anker erneut fokussiert. */
  revision: number;
}

export const useConfigDialog = create<ConfigDialogState>(() => ({ open: false, anchor: null, diff: false, revision: 0 }));

/** Öffnet den Konfigurationsdialog, auf Wunsch direkt bei einem Feld oder einer Seite. */
export function openConfig(opts: { anchor?: string; diff?: boolean } = {}): void {
  useConfigDialog.setState((s) => ({ open: true, anchor: opts.anchor ?? null, diff: opts.diff ?? false, revision: s.revision + 1 }));
}

export function closeConfig(): void {
  useConfigDialog.setState({ open: false, anchor: null, diff: false });
}
