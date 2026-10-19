// Statusautomat und Statustexte der Integrationen (INTEG-001, INTEG-090, INTEG-110, INTEG-130).
// Reine Logik: der Kern liefert den Zustand (`ApiSourceInfo.state`), die Oberfläche lässt nur die erlaubten Übergänge zu.
import type { ApiConnState } from "../../ipc/gen/ApiConnState";
import type { CmdError } from "../../ipc/gen/CmdError";
import { describeError } from "../../ipc/errors";
import type { TFn } from "../../i18n";

export type ConnEvent = "connect" | "succeeded" | "failed" | "disconnect";

/** Die einzigen erlaubten Übergänge (INTEG-001); alles andere liefert `null`. */
const TRANSITIONS: Record<ApiConnState, Partial<Record<ConnEvent, ApiConnState>>> = {
  disconnected: { connect: "connecting" },
  connecting: { succeeded: "connected", failed: "error" },
  connected: { disconnect: "disconnected" },
  error: { disconnect: "disconnected" },
};

export function transition(state: ApiConnState, event: ConnEvent): ApiConnState | null {
  return TRANSITIONS[state][event] ?? null;
}

/** Bedienbare Schaltflächen je Zustand: „Verbinden“ nur ohne Verbindung, „Trennen“ bei Verbindung oder Fehler. */
export function allowedActions(state: ApiConnState): { connect: boolean; disconnect: boolean } {
  return { connect: transition(state, "connect") !== null, disconnect: transition(state, "disconnect") !== null };
}

/** Gründe, aus denen Google einen PSI-Schlüssel ablehnt (INTEG-130). */
const PSI_DENIAL = /\b(SERVICE_DISABLED|API_KEY_INVALID|API_KEY_SERVICE_BLOCKED|API_KEY_HTTP_REFERRER_BLOCKED)\b/;

/** Ablehnungsgrund eines PSI-Schlüssels oder `null`. */
export function psiDenialReason(error: CmdError | null | undefined): string | null {
  if (!error) return null;
  const sources = [error.detail ?? "", ...Object.values(error.args ?? {}).map((v) => (typeof v === "string" ? v : ""))];
  for (const s of sources) {
    const hit = PSI_DENIAL.exec(s);
    if (hit) return hit[1] as string;
  }
  return null;
}

/** HTTP-Code und Anbietermeldung aus dem redigierten Detail `HTTP <code>: <Meldung>` (os-integrations). */
export function providerFailure(error: CmdError): { code: number; message: string } | null {
  const hit = /^HTTP (\d{3}):\s*(.*)$/s.exec(error.detail ?? "");
  return hit ? { code: Number(hit[1]), message: (hit[2] ?? "").trim() } : null;
}

/**
 * Meldungsteil von `Fehler: <Meldung>`: Anbieterantwort als `<HTTP-Code> <Meldung des Anbieters>`, PSI-Ablehnung mit
 * Grund, sonst der übersetzte Kernfehler (z. B. Zeitüberschreitung nach 30 s).
 */
export function errorMessage(t: TFn, kind: string, error: CmdError, has?: (key: string) => boolean): string {
  if (kind === "psi") {
    const reason = psiDenialReason(error);
    if (reason) return t("integ-psi-denied", { reason });
  }
  const provider = providerFailure(error);
  if (provider) return provider.message ? `${provider.code} ${provider.message}` : String(provider.code);
  return describeError(t, error, has).text;
}

export interface StatusInput {
  state: ApiConnState;
  /** Konto und Auswahl in Anzeigereihenfolge, z. B. Konto / Property / Stream (INTEG-090). */
  parts?: readonly (string | null | undefined)[];
  error?: CmdError | null;
}

/** Genau einer der vier Status (INTEG-001). */
export function statusText(t: TFn, kind: string, input: StatusInput, has?: (key: string) => boolean): string {
  switch (input.state) {
    case "disconnected":
      return t("integ-status-disconnected");
    case "connecting":
      return t("integ-status-connecting");
    case "connected": {
      const parts = (input.parts ?? []).filter((p): p is string => !!p && p.trim() !== "");
      return parts.length > 0 ? t("integ-status-connected-as", { account: parts.join(" / ") }) : t("integ-status-connected");
    }
    case "error":
      return t("integ-status-error", { message: input.error ? errorMessage(t, kind, input.error, has) : t("integ-error-unknown") });
  }
}

/** Maskierte Anzeige eines Schlüssels: acht Punkte und die letzten vier Zeichen (INTEG-002). */
export function maskedKey(suffix: string | null | undefined): string | null {
  if (!suffix) return null;
  return `••••••••${suffix.slice(-4)}`;
}
