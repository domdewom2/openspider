import { toCmdError } from "../../ipc/errors";
import { useSession } from "../../state/session";

/** Command failures and pushed notices use the same queue and dialog presentation. */
export function reportError(error: unknown, extra: Record<string, string> = {}): void {
  const e = toCmdError(error);
  if (e.code === "Cancelled" || e.code === "Stale") return;
  useSession.getState().pushNotice({ level: "error", code: e.messageKey, args: {
    ...e.args, ...extra, errorCode: e.code, ...(e.detail ? { detail: e.detail } : {}),
  } });
}
