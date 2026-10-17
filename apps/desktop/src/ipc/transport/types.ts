import type { UiEvent } from "../gen/UiEvent";

export type { UiEvent };

/** Ein Transport trägt Commands und Ereignisse; Tauri, Bridge und Mock stehen hinter dieser Schnittstelle. */
export interface Transport {
  readonly name: "tauri" | "bridge" | "mock";
  invoke<T>(cmd: string, args: unknown, signal?: AbortSignal): Promise<T>;
  /** Ein Ereigniskanal je Fenster (M2-ui-concept 2.5); liefert die Abmeldung. */
  subscribe(window: string, onEvent: (e: UiEvent) => void): () => void;
}
