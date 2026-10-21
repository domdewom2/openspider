// Konsolenmeldungen (UI-040, TECH-114): Spalten, Level und Kürzung langer Nachrichten.
export const CONSOLE_COLUMNS = ["Level", "Message", "Source URL", "Line", "Column", "Timestamp"] as const;
export const CONSOLE_LEVELS = ["error", "warning", "info", "log"] as const;
/** Längere Nachrichten werden gekürzt angezeigt und bleiben vollständig kopierbar (UI-040). */
export const MESSAGE_LIMIT = 10_000;
export function shortenMessage(message: string): { text: string; total: number; truncated: boolean } {
  const chars = Array.from(message);
  return chars.length <= MESSAGE_LIMIT ? { text: message, total: chars.length, truncated: false } : { text: chars.slice(0, MESSAGE_LIMIT).join("") + "…", total: chars.length, truncated: true };
}
/** Bulk-Export „JavaScript → Pages With JavaScript Issues“ (OUT-058); der Katalog des Kerns nennt die tatsächlichen Namen. */
export function findBulk(names: readonly string[]): string | undefined {
  return names.find(n => n === "JavaScript:Pages With JavaScript Issues") ?? names.find(n => /Pages With JavaScript Issues/i.test(n)) ?? names.find(n => n === "JavaScript:All Console Messages");
}
/** Bericht „JavaScript → JavaScript Console Log Summary“ (OUT-107). */
export function findReport(names: readonly string[]): string | undefined {
  return names.find(n => /Console Log Summary/i.test(n));
}
