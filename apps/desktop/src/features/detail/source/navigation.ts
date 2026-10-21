// Übergabe zwischen Fach-Tabs (UI-048 → UI-041): „Im gerenderten HTML anzeigen“ öffnet View Source mit markiertem Element.
import { create } from "zustand";
import { showDetailTab } from "../frame/selection";

export interface SourceTarget {
  /** Fortlaufende Nummer; View Source verarbeitet jede Anfrage genau einmal. */
  id: number;
  urlId: number;
  crawlId?: string;
  which: "original" | "rendered";
  /** HTML-Ausschnitt der betroffenen Stelle und CSS-Selektor (Hervorhebung durch Textsuche im Quelltext). */
  snippet: string;
  selector: string;
}
let counter = 0;
export const useSourceTarget = create<{ target?: SourceTarget }>(() => ({}));
export function showInSource(target: Omit<SourceTarget, "id">): void {
  useSourceTarget.setState({ target: { ...target, id: ++counter } });
  showDetailTab("source");
}
