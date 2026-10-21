// Ergebnisarten, Spalten und Sortierung der Accessibility-Details (UI-048, TECH-259).
import type { SortKey } from "../../../ipc/gen/SortKey";

export const A11Y_RESULTS = ["Violation", "Incomplete", "Inapplicable", "Passed", "All"] as const;
export type A11yResult = typeof A11Y_RESULTS[number];
export const RULE_COLUMNS = ["Issue", "Result", "Guideline", "Impact", "Nodes", "Help URL"] as const;
export const NODE_COLUMNS = ["Address", "Location on Page", "HTML"] as const;
/** Impact (Critical → Minor), dann Nodes absteigend, dann Issue alphabetisch (OS-Festlegung UI-048); die Rangfolge der Auswirkung wendet der Kern an. */
export const DEFAULT_RULE_SORT: SortKey[] = [{ column: "Impact", dir: "asc" }, { column: "Nodes", dir: "desc" }, { column: "Issue", dir: "asc" }];
