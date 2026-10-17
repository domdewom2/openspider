// Sitzungszustand (Gerüst): vom Kern gespiegelt, ausschließlich durch Tick-Ereignisse geschrieben (M2-ui-concept 2.2).
// Komponenten lesen mit Selektoren; spätere Pakete (U03 Kopf-/Statusleiste, U12 Seitenleiste) erweitern die Selektoren.
import { create } from "zustand";
import type { EventBus } from "../ipc/events";
import type { AnalysisStatus } from "../ipc/gen/AnalysisStatus";
import type { CrawlStatus } from "../ipc/gen/CrawlStatus";
import type { McpStatus } from "../ipc/gen/McpStatus";
import type { Notice } from "../ipc/gen/Notice";
import type { OpStatus } from "../ipc/gen/OpStatus";
import type { Tick } from "../ipc/gen/Tick";
import type { ViewChanged } from "../ipc/gen/ViewChanged";

export interface SessionState {
  /** Laufende Nummer des letzten angewendeten Ticks (ältere werden verworfen). */
  seq: number;
  crawl: CrawlStatus | null;
  analysis: AnalysisStatus | null;
  mcp: McpStatus | null;
  countsVersion: number;
  issuesVersion: number;
  sidebarVersion: number;
  /** Zuletzt gemeldete Version je View (Datenquellen laden bei Wechsel nach). */
  views: Record<string, ViewChanged>;
  ops: Record<string, OpStatus>;
  /** Neue Meldungen des letzten Ticks (Startfehler, Plattenplatz, Crawl-Ende …); Verbraucher quittieren sie. */
  notices: Notice[];
  applyTick(tick: Tick): void;
  clearNotices(): void;
}

export const useSession = create<SessionState>((set, get) => ({
  seq: -1,
  crawl: null,
  analysis: null,
  mcp: null,
  countsVersion: 0,
  issuesVersion: 0,
  sidebarVersion: 0,
  views: {},
  ops: {},
  notices: [],
  applyTick(tick) {
    if (tick.seq <= get().seq) return;
    set((s) => {
      const views = { ...s.views };
      for (const v of tick.views) views[v.viewId] = v;
      const ops = { ...s.ops };
      for (const o of tick.ops) ops[o.opId] = o;
      return {
        seq: tick.seq,
        crawl: tick.crawl ?? s.crawl,
        analysis: tick.analysis ?? s.analysis,
        mcp: tick.mcp ?? s.mcp,
        countsVersion: tick.countsVersion,
        issuesVersion: tick.issuesVersion,
        sidebarVersion: tick.sidebarVersion,
        views,
        ops,
        notices: tick.notices.length > 0 ? [...s.notices, ...tick.notices] : s.notices,
      };
    });
  },
  clearNotices() {
    set({ notices: [] });
  },
}));

/** Verbindet den Event-Bus des Fensters mit dem Sitzungszustand; liefert die Abmeldung. */
export function attachSession(bus: EventBus): () => void {
  return bus.on("tick", (t) => useSession.getState().applyTick(t));
}
