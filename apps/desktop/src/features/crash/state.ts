// Absturzberichte (OUT-246/OUT-247): Hinweis beim Start, Ansehen, Senden nur nach Klick, Liste im Hilfe-Menü.
import { create } from "zustand";
import { crash } from "../../ipc/crash";
import type { CrashOverview } from "../../ipc/gen/CrashOverview";
import type { CrashSendFailure } from "../../ipc/gen/CrashSendFailure";
import type { CrashSendResult } from "../../ipc/gen/CrashSendResult";
import { toCmdError } from "../../ipc/errors";

export type SendState =
  | { kind: "idle" }
  | { kind: "sending" }
  | { kind: "done"; refs: string[] }
  | { kind: "failed"; failure: CrashSendFailure | null; messageKey?: string };

export interface ViewRequest {
  /** Berichte, zwischen denen der Dialog wechselt (neueste zuerst). */
  ids: string[];
  selected: string;
  /** Aus dem Hinweis geöffnet: „Senden“ sendet wie der Hinweis. */
  fromHint: boolean;
}

interface CrashState {
  overview: CrashOverview | null;
  /** Berichte des Hinweises (Stand beim ersten Laden). */
  hint: string[];
  hintOpen: boolean;
  hintSend: SendState;
  view: ViewRequest | null;
  listOpen: boolean;
  /** Ergebnis je Bericht in der Liste. */
  rowSend: Record<string, SendState>;
  /** Erste erfolgreiche Übersicht geladen (Grundlage des Hinweises). */
  loaded: boolean;
}

const initial = (): CrashState => ({
  overview: null,
  hint: [],
  hintOpen: false,
  hintSend: { kind: "idle" },
  view: null,
  listOpen: false,
  rowSend: {},
  loaded: false,
});

export const useCrash = create<CrashState>(initial);

export function resetCrashState(): void {
  useCrash.setState(initial());
}

/** Lädt die Übersicht; beim ersten Laden entsteht daraus der Hinweis (nur Berichte früherer Läufe). */
export async function loadOverview(signal?: AbortSignal): Promise<CrashOverview | null> {
  try {
    const overview = await crash.overview(signal);
    const first = !useCrash.getState().loaded;
    useCrash.setState({
      overview,
      loaded: true,
      ...(first ? { hint: overview.pending, hintOpen: overview.pending.length > 0 } : {}),
    });
    return overview;
  } catch {
    // Ohne Kern-Command (ältere Bridge, Mock) oder bei Abbruch bleibt alles still; der Hinweis entsteht erst aus der
    // ersten erfolgreichen Antwort (StrictMode bricht den ersten Aufruf ab).
    return null;
  }
}

function outcome(result: CrashSendResult): SendState {
  const failed = result.results.find((r) => r.failure);
  if (failed) return { kind: "failed", failure: failed.failure ?? null };
  const refs = result.results.flatMap((r) => (r.serverRef ? [r.serverRef] : []));
  return refs.length ? { kind: "done", refs } : { kind: "failed", failure: null };
}

async function sendIds(ids: string[], note: string, email: string): Promise<SendState> {
  try {
    return outcome(await crash.send({ ids, note, email }));
  } catch (e) {
    return { kind: "failed", failure: null, messageKey: toCmdError(e).messageKey };
  }
}

/** Ungesendete Berichte des Hinweises (für „Erneut versuchen“). */
function unsentHint(): string[] {
  const { hint, overview } = useCrash.getState();
  const sent = new Set((overview?.reports ?? []).filter((r) => r.sent).map((r) => r.id));
  return hint.filter((id) => !sent.has(id));
}

/** „Senden“ im Hinweis bzw. im Dialog aus dem Hinweis: die neuesten 5, einzeln nacheinander (Kern). */
export async function sendHint(note = "", email = ""): Promise<SendState> {
  const ids = unsentHint();
  useCrash.setState({ hintSend: { kind: "sending" } });
  const state = await sendIds(ids, note, email);
  useCrash.setState({ hintSend: state });
  await loadOverview();
  return state;
}

/** „Nicht senden“: Hinweis für diese Berichte dauerhaft ausblenden. */
export async function dismissHint(): Promise<void> {
  const ids = useCrash.getState().hint;
  useCrash.setState({ hintOpen: false });
  try {
    await crash.dismiss({ ids });
  } catch {
    /* Beim nächsten Start erscheint der Hinweis erneut. */
  }
  await loadOverview();
}

/** „Senden“ eines einzelnen Berichts aus der Liste oder dem Dialog der Liste. */
export async function sendOne(id: string, note = "", email = ""): Promise<SendState> {
  useCrash.setState((s) => ({ rowSend: { ...s.rowSend, [id]: { kind: "sending" } } }));
  const state = await sendIds([id], note, email);
  useCrash.setState((s) => ({ rowSend: { ...s.rowSend, [id]: state } }));
  await loadOverview();
  return state;
}

export async function deleteReport(id: string): Promise<void> {
  await crash.delete({ id });
  await loadOverview();
}

export function openList(): void {
  useCrash.setState({ listOpen: true, rowSend: {} });
  void loadOverview();
}

export function openView(view: ViewRequest): void {
  useCrash.setState({ view });
}
