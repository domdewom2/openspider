import type { ApiKind } from "../../../ipc/gen/ApiKind";
import type { ApiRow } from "../../../ipc/gen/ApiRow";

/** Fixed row order (UI-070); Majestic, Ahrefs and Moz are always three separate rows. */
export const API_ORDER = ["ga4", "gsc", "psi", "majestic", "ahrefs", "moz"] as const satisfies readonly ApiKind[];
export type ApiPanelKind = (typeof API_ORDER)[number];

export type ApiStatus = "disconnected" | "connected" | "running" | "done" | "error" | "quotaExhausted";

const blank = (kind: ApiKind): ApiRow => ({ kind, labelKey: `api-${kind}`, connected: false, requested: 0, received: 0, errors: 0, running: false, completed: 0 });

/** Every fixed row exists, also when the core reports nothing for a source (then “not connected”). */
export function panelRows(rows: readonly ApiRow[]): ApiRow[] {
  return API_ORDER.map(kind => rows.find(r => r.kind === kind) ?? blank(kind));
}

/**
 * Status of a source. Quota wins over everything else once connected; “error” means requests failed without a
 * single success; “done” means every requested URL is settled (data, no data or final error, INTEG-007).
 */
export function apiStatus(row: ApiRow): ApiStatus {
  if (!row.connected) return "disconnected";
  if (row.quotaLeft != null && row.quotaLeft <= 0) return "quotaExhausted";
  if (row.running) return "running";
  if (row.errors > 0 && row.received === 0) return "error";
  if (row.requested > 0 && row.completed >= row.requested) return "done";
  return "connected";
}

/** Progress = settled / requested URLs, capped at 100 (INTEG-007: errors count as settled). */
export function progressPercent(row: ApiRow): number {
  return row.requested > 0 ? Math.min(100, Math.floor((Math.min(row.completed, row.requested) * 100) / row.requested)) : 0;
}

const SCHEME = /\b(bearer|basic)\s+[A-Za-z0-9._~+/=-]{6,}/gi;
const PAIR = /\b(access[_-]?token|refresh[_-]?token|id[_-]?token|api[_-]?key|apikey|key|token|secret|password|client[_-]?secret|authorization)(["']?\s*[=:]\s*["']?)(?!•)[^\s&"',;]+/gi;

/** Defence in depth: the core redacts messages, the tab masks anything that still looks like a credential. */
export function redact(message: string): string {
  return message.replace(SCHEME, (_m, scheme: string) => `${scheme} ••••`).replace(PAIR, (_m, name: string, sep: string) => `${name}${sep}••••`);
}
