// Einstellungen je Quelle (`api.getSettings`/`api.setSettings`, Abschnitt `api.<quelle>` der Konfiguration, nie
// Geheimnisse). Änderungen an Daten-Einstellungen fragen nach „Löschen und neu laden“ bzw. „Behalten“ (INTEG-010).
import { api } from "../../ipc/api";
import type { ApiKind } from "../../ipc/gen/ApiKind";
import type { ApiSettingsDecision } from "../../ipc/gen/ApiSettingsDecision";
import type { ApiSettingsResult } from "../../ipc/gen/ApiSettingsResult";
import type { JsonValue } from "../../ipc/gen/serde_json/JsonValue";

export type Values = { [key: string]: JsonValue };

export function asValues(value: JsonValue | undefined): Values {
  return typeof value === "object" && value !== null && !Array.isArray(value) ? value : {};
}

export function getIn(values: Values, path: readonly string[]): JsonValue | undefined {
  let cur: JsonValue | undefined = values;
  for (const key of path) {
    if (typeof cur !== "object" || cur === null || Array.isArray(cur)) return undefined;
    cur = cur[key];
  }
  return cur;
}

/** Unveränderliches Setzen eines verschachtelten Werts. */
export function setIn(values: Values, path: readonly string[], value: JsonValue): Values {
  const [head, ...rest] = path;
  if (head === undefined) return values;
  return { ...values, [head]: rest.length === 0 ? value : setIn(asValues(values[head]), rest, value) };
}

export const str = (v: JsonValue | undefined): string | null => (typeof v === "string" && v !== "" ? v : null);
export const strings = (v: JsonValue | undefined): string[] => (Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : []);
export const bool = (v: JsonValue | undefined): boolean => v === true;

export async function loadSettings(kind: ApiKind): Promise<Values> {
  return asValues((await api.getSettings({ kind })).values);
}

/** Speichert; `ask` liefert bei vorhandenen Daten `confirmationRequired` statt still zu verwerfen (INTEG-010). */
export function saveSettings(kind: ApiKind, values: Values, decision: ApiSettingsDecision = "ask"): Promise<ApiSettingsResult> {
  return api.setSettings({ kind, settings: { kind, values }, decision });
}

/** Kennzahlen aus „Erweitert“ (INTEG-005): gleichzeitige Anfragen 1–50, Wiederholungen 0–20. */
export const LIMITS = {
  concurrency: { min: 1, max: 50 },
  retries: { min: 0, max: 20 },
} as const;

export function limitError(name: keyof typeof LIMITS, raw: string): boolean {
  const n = Number(raw);
  const { min, max } = LIMITS[name];
  return !/^\d+$/.test(raw.trim()) || n < min || n > max;
}
