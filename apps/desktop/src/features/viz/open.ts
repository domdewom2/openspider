// Öffnen von Visualisierungsfenstern (UI-090): Menü „Visualisierungen“ (Wurzel = Start-URL) oder Kontextmenü einer URL
// (Wurzel = diese URL). Der Kern legt den Schnappschuss an und öffnet in der App das Fenster selbst; im Browser
// (E2E-A, Harness) öffnet die Oberfläche ein benanntes Fenster. Die Art reist über den Query-Parameter `kind` und – weil
// die Hülle nur `snapshotId` übergibt – zusätzlich über den gemeinsamen Speicher der Fenster.
import type { VizKind } from "../../ipc/gen/VizKind";
import type { VizOpenResult } from "../../ipc/gen/VizOpenResult";
import { isTauri } from "../../ipc/transport/tauri";
import { viz } from "../../ipc/viz";
import { windowLabelFor } from "./labels";

/** Arten, die dieses Paket darstellt (UI-091, UI-092, UI-093, UI-099). */
export type VizWindowKind = "crawlTree" | "crawlGraph" | "directoryTree" | "anchorWords";
export const WINDOW_KINDS: readonly VizWindowKind[] = ["crawlTree", "crawlGraph", "directoryTree", "anchorWords"];
export const GRAPH_KINDS: readonly VizWindowKind[] = ["crawlTree", "crawlGraph", "directoryTree"];

export function isWindowKind(k: string | null | undefined): k is VizWindowKind {
  return !!k && (WINDOW_KINDS as readonly string[]).includes(k);
}

export function layoutOf(kind: VizWindowKind): "tree" | "force" | "words" {
  return kind === "crawlGraph" ? "force" : kind === "anchorWords" ? "words" : "tree";
}

/** Kernart für `viz.graph` (der kraftgerichtete Graph nutzt die Entdeckungsstruktur des Crawl-Baums). */
export function coreKind(kind: VizWindowKind): VizKind {
  return kind;
}

export { windowLabelFor };

export const KIND_KEY = "os.viz.kind.";

export interface KindStore {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

function safeStorage(): KindStore | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export interface VizWindowParams {
  snapshotId: string | null;
  kind: VizWindowKind | null;
  rootUrlId: number | null;
  label: string;
}

/** Parameter eines Visualisierungsfensters aus Query und gemeinsamem Speicher. */
export function readWindowParams(search: string, store: KindStore | null = safeStorage()): VizWindowParams {
  const q = new URLSearchParams(search);
  const snapshotId = q.get("snapshotId");
  const fromQuery = q.get("kind");
  const stored = snapshotId ? store?.getItem(KIND_KEY + snapshotId) : null;
  const kind = isWindowKind(fromQuery) ? fromQuery : isWindowKind(stored) ? stored : null;
  const rawRoot = q.get("root");
  const root = rawRoot !== null && rawRoot !== "" ? Number(rawRoot) : Number.NaN;
  return {
    snapshotId,
    kind,
    rootUrlId: Number.isFinite(root) ? root : null,
    label: q.get("label") ?? (snapshotId ? windowLabelFor(snapshotId) : "viz-1"),
  };
}

/** Wartet bis `timeoutMs` auf die Art aus einem anderen Fenster (die Hülle öffnet das Fenster vor der Antwort). */
export function waitForKind(snapshotId: string, timeoutMs = 2_000, store: KindStore | null = safeStorage()): Promise<VizWindowKind | null> {
  const now = store?.getItem(KIND_KEY + snapshotId);
  if (isWindowKind(now)) return Promise.resolve(now);
  if (typeof window === "undefined") return Promise.resolve(null);
  return new Promise((resolve) => {
    const done = (k: VizWindowKind | null) => {
      window.removeEventListener("storage", onStorage);
      clearTimeout(timer);
      resolve(k);
    };
    const onStorage = (e: StorageEvent) => {
      if (e.key === KIND_KEY + snapshotId && isWindowKind(e.newValue)) done(e.newValue);
    };
    const timer = setTimeout(() => {
      const late = store?.getItem(KIND_KEY + snapshotId);
      done(isWindowKind(late) ? late : null);
    }, timeoutMs);
    window.addEventListener("storage", onStorage);
  });
}

export interface OpenDeps {
  open: (args: { kind: VizKind; rootUrlId?: number | null }) => Promise<VizOpenResult>;
  /** In der App öffnet der Kern das Fenster; sonst die Oberfläche. */
  hostOpensWindow: boolean;
  openWindow: (url: string, name: string) => void;
  store: KindStore | null;
  /** Aktuelle Query (Transport `bridge`/`transport` wird übernommen). */
  search: string;
}

function defaultDeps(): OpenDeps {
  return {
    open: (args) => viz.open(args),
    hostOpensWindow: isTauri(),
    openWindow: (url, name) => {
      window.open(url, name, "popup,width=1100,height=760")?.focus();
    },
    store: safeStorage(),
    search: typeof location !== "undefined" ? location.search : "",
  };
}

/** Öffnet eine Visualisierung; `rootUrlId` = Wurzel (Kontextmenü) bzw. Ziel-URL der Wortwolke. */
export async function openVisualisation(kind: VizWindowKind, rootUrlId: number | null = null, deps: Partial<OpenDeps> = {}): Promise<VizOpenResult> {
  const d = { ...defaultDeps(), ...deps };
  const res = await d.open({ kind: coreKind(kind), rootUrlId });
  try {
    d.store?.setItem(KIND_KEY + res.snapshotId, kind);
  } catch {
    // Speicher voll oder gesperrt: das Fenster fällt dann auf seine Artauswahl zurück.
  }
  if (!d.hostOpensWindow) {
    const q = new URLSearchParams();
    const cur = new URLSearchParams(d.search);
    for (const k of ["bridge", "transport"]) {
      const v = cur.get(k);
      if (v !== null) q.set(k, v);
    }
    q.set("label", res.window);
    q.set("snapshotId", res.snapshotId);
    q.set("kind", kind);
    if (rootUrlId !== null) q.set("root", String(rootUrlId));
    d.openWindow(`viz.html?${q.toString()}`, res.window);
  }
  return res;
}

/** Kanal zwischen Visualisierungs- und Hauptfenster (gleicher Ursprung, ohne Kern). */
export const CHANNEL = "os-viz";

export interface AnchorRequest {
  type: "inlinksAnchor";
  urlId: number;
  anchor: string;
}

/** UI-099: Klick auf einen Begriff → Inlinks der URL gefiltert auf diesen Ankertext im Hauptfenster. */
export function requestInlinksForAnchor(urlId: number, anchor: string): void {
  if (typeof BroadcastChannel === "undefined") return;
  const ch = new BroadcastChannel(CHANNEL);
  ch.postMessage({ type: "inlinksAnchor", urlId, anchor } satisfies AnchorRequest);
  ch.close();
}

/** Empfang im Hauptfenster: der Besitzer der Inlinks-Ansicht (Detailfenster) meldet sich hier an. */
export function onInlinksAnchorRequest(cb: (req: AnchorRequest) => void): () => void {
  if (typeof BroadcastChannel === "undefined") return () => undefined;
  const ch = new BroadcastChannel(CHANNEL);
  ch.onmessage = (e: MessageEvent<AnchorRequest>) => {
    if (e.data?.type === "inlinksAnchor" && Number.isFinite(e.data.urlId) && typeof e.data.anchor === "string") cb(e.data);
  };
  return () => ch.close();
}
