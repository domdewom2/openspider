// Verbindungen der Integrationen: `api.list` als Wahrheit, `api.connect` als lang laufende Operation (Ergebnis über
// `Tick.ops`), Trennen und Konto entfernen. Zwischen Klick und Antwort zeigt die Oberfläche `Verbinde…` (INTEG-001).
import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../../ipc/api";
import { toCmdError } from "../../ipc/errors";
import type { ApiConnectArgs } from "../../ipc/gen/ApiConnectArgs";
import type { ApiConnState } from "../../ipc/gen/ApiConnState";
import type { ApiKind } from "../../ipc/gen/ApiKind";
import type { ApiSourceInfo } from "../../ipc/gen/ApiSourceInfo";
import type { CmdError } from "../../ipc/gen/CmdError";
import { useSession } from "../../state/session";
import { transition } from "./status";

export interface ApiSources {
  sources: ApiSourceInfo[] | null;
  /** Fehler beim Laden der Liste bzw. einer Aktion. */
  error: CmdError | null;
  /** Zustand je Quelle einschließlich laufender Verbindungsversuche. */
  stateOf(kind: ApiKind): ApiConnState;
  /** Fehler des letzten Verbindungsversuchs (aus dem Kern oder aus der Operation). */
  errorOf(kind: ApiKind): CmdError | null;
  reload(): Promise<void>;
  connect(args: ApiConnectArgs): Promise<void>;
  disconnect(kind: ApiKind): Promise<void>;
  removeAccount(kind: ApiKind, accountId: string): Promise<void>;
}

export function useApiSources(): ApiSources {
  const [sources, setSources] = useState<ApiSourceInfo[] | null>(null);
  const [error, setError] = useState<CmdError | null>(null);
  const [pending, setPending] = useState<Partial<Record<ApiKind, string>>>({});
  const [failures, setFailures] = useState<Partial<Record<ApiKind, CmdError>>>({});
  const ops = useSession((s) => s.ops);
  const live = useRef(true);

  const reload = useCallback(async () => {
    try {
      const list = await api.list();
      if (live.current) {
        setSources(list);
        setError(null);
      }
    } catch (e) {
      if (live.current) setError(toCmdError(e));
    }
  }, []);

  useEffect(() => {
    live.current = true;
    void reload();
    return () => {
      live.current = false;
    };
  }, [reload]);

  // Ende eines Verbindungsversuchs: Liste neu laden, Fehler der Operation merken.
  useEffect(() => {
    const finished = Object.entries(pending).filter(([, opId]) => {
      const status = opId ? ops[opId] : undefined;
      return status && status.state !== "running";
    }) as [ApiKind, string][];
    if (finished.length === 0) return;
    setPending((p) => {
      const next = { ...p };
      for (const [kind] of finished) delete next[kind];
      return next;
    });
    setFailures((f) => {
      const next = { ...f };
      for (const [kind, opId] of finished) {
        const status = ops[opId];
        if (status?.state === "failed" && status.error) next[kind] = status.error;
        else delete next[kind];
      }
      return next;
    });
    void reload();
  }, [ops, pending, reload]);

  const info = (kind: ApiKind) => sources?.find((s) => s.kind === kind);
  const stateOf = (kind: ApiKind): ApiConnState => (pending[kind] ? "connecting" : (info(kind)?.state ?? "disconnected"));
  const errorOf = (kind: ApiKind): CmdError | null => info(kind)?.error ?? failures[kind] ?? null;

  const connect = async (args: ApiConnectArgs) => {
    if (transition(stateOf(args.kind), "connect") === null) return;
    setFailures((f) => ({ ...f, [args.kind]: undefined }));
    setPending((p) => ({ ...p, [args.kind]: "starting" }));
    try {
      const handle = await api.connect(args);
      // Ab hier wartet der Effekt oben auf das Ende der Operation im Tick.
      setPending((p) => ({ ...p, [args.kind]: handle.opId }));
    } catch (e) {
      setPending((p) => {
        const next = { ...p };
        delete next[args.kind];
        return next;
      });
      setFailures((f) => ({ ...f, [args.kind]: toCmdError(e) }));
      await reload();
    }
  };

  const disconnect = async (kind: ApiKind) => {
    if (transition(stateOf(kind), "disconnect") === null) return;
    try {
      await api.disconnect({ kind });
      setFailures((f) => ({ ...f, [kind]: undefined }));
    } catch (e) {
      setError(toCmdError(e));
    }
    await reload();
  };

  const removeAccount = async (kind: ApiKind, accountId: string) => {
    try {
      await api.removeAccount({ kind, accountId });
    } catch (e) {
      setError(toCmdError(e));
    }
    await reload();
  };

  return { sources, error, stateOf, errorOf, reload, connect, disconnect, removeAccount };
}
