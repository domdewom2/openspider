// Fehlerumsetzung (M2-ui-concept 2.6, 4.2): CmdError aus dem Kern → IpcError, Klassen und Darstellung.
import type { CmdError } from "./gen/CmdError";
import type { ErrorCode } from "./gen/ErrorCode";

export type { CmdError, ErrorCode };

const ERROR_CODES: readonly ErrorCode[] = [
  "InvalidInput",
  "NotFound",
  "Stale",
  "Cancelled",
  "Busy",
  "Timeout",
  "Io",
  "Permission",
  "Unsupported",
  "NoCrawl",
  "CrawlRunning",
  "Store",
  "Engine",
  "Internal",
];

/** Fehler eines Commands als Exception; `cmdError` ist das unveränderte Kern-Format. */
export class IpcError extends Error {
  readonly cmdError: CmdError;
  constructor(cmdError: CmdError) {
    super(cmdError.detail ?? cmdError.messageKey);
    this.name = "IpcError";
    this.cmdError = cmdError;
  }
  get code(): ErrorCode {
    return this.cmdError.code;
  }
}

export function isCmdError(value: unknown): value is CmdError {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return typeof v.code === "string" && (ERROR_CODES as readonly string[]).includes(v.code) && typeof v.messageKey === "string";
}

/** Macht aus beliebigen Fehlern (Tauri-Ablehnung, WebSocket, Exception) ein `CmdError`. */
export function toCmdError(e: unknown): CmdError {
  if (e instanceof IpcError) return e.cmdError;
  if (isCmdError(e)) return { ...e, args: e.args ?? {}, retryable: e.retryable ?? false };
  if (e instanceof DOMException && e.name === "AbortError") return localError("Cancelled", "err-cancelled");
  if (e instanceof Error) return { ...localError("Internal", "err-internal"), detail: e.message };
  if (typeof e === "string") return { ...localError("Internal", "err-internal"), detail: e };
  return localError("Internal", "err-internal");
}

export function localError(code: ErrorCode, messageKey: string, args: Record<string, unknown> = {}): CmdError {
  return { code, messageKey, args: args as CmdError["args"], retryable: false };
}

export function toIpcError(e: unknown): IpcError {
  return e instanceof IpcError ? e : new IpcError(toCmdError(e));
}

/** Darstellungsklassen laut 2.6. */
export type ErrorClass = "validation" | "silent" | "banner" | "fatal";

export function classifyError(e: CmdError): ErrorClass {
  switch (e.code) {
    case "InvalidInput":
      return "validation";
    case "Stale":
    case "Cancelled":
      return "silent";
    case "Internal":
      return "fatal";
    default:
      return "banner";
  }
}

/** Veraltete und abgebrochene Antworten werden still verworfen und nie angezeigt. */
export function isSilentError(e: unknown): boolean {
  return classifyError(toCmdError(e)) === "silent";
}

export type TranslateFn = (key: string, args?: Record<string, unknown>) => string;

/** Text für den Nutzer: `messageKey` + Argumente; `detail` (englisch, technisch) nur getrennt, nie im Haupttext. */
export function describeError(t: TranslateFn, e: CmdError, has?: (key: string) => boolean): { text: string; detail?: string } {
  const known = has ? has(e.messageKey) : true;
  const args = { ...(e.args as Record<string, unknown>), code: e.code };
  const text = known ? t(e.messageKey, args) : t(`err-code-${e.code.toLowerCase()}`, args);
  return e.detail ? { text, detail: e.detail } : { text };
}
