// Baum- und Graphansicht eines Visualisierungsfensters (UI-090 … UI-097, UI-146): lädt den Schnappschuss über
// `viz.graph`, legt Baum (d3-hierarchy) bzw. Graph (Worker) an, zeichnet auf Canvas und bietet Suche, Kontextmenü,
// Knotenbudget, Teilbaum öffnen, Wurzel setzen mit Pfadleiste, Links eines Knotens einblenden sowie SVG/PNG/CSV-Export.
// Der Verzeichnisgraph (UI-094) nutzt dieselbe Ansicht mit den Daten des Verzeichnisbaums.
import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { getFormatters, useT } from "../../i18n";
import { describeError, toCmdError } from "../../ipc/errors";
import { eventBus } from "../../ipc/events";
import type { GraphDto } from "../../ipc/gen/GraphDto";
import type { OpStatus } from "../../ipc/gen/OpStatus";
import { isTauri } from "../../ipc/transport/tauri";
import { viz } from "../../ipc/viz";
import { Banner, Button, ContextMenu, Dialog, DialogFooter, type MenuEntry } from "../../ui/primitives";
import { CompactLegend } from "./CompactLegend";
import { GraphCanvas, type GraphCanvasHandle } from "./GraphCanvas";
import { PALETTES } from "./model/encoding";
import { chooseFolder, saveBlob, svgBlob, useRenderStyle } from "./exporters";
import { csvPayload, directoryPath, exploreDecision, RootTrail, tooltipRows } from "./model/details";
import type { ForceInput, ForceReply } from "./model/force";
import { createForceRunner, type ForceRunner } from "./model/forceRunner";
import { linkMenuEntries, overlayCsvEdges, overlayFrom, sameStructure, treeEdges, withLinks, type LinkOverlay, type LinkRequest } from "./model/links";
import { applyPositions, buildScene, sceneOverlay, type Scene } from "./model/scene";
import { current as currentMatch, EMPTY_SEARCH, search as runSearch, step as stepSearch, type SearchState } from "./model/search";
import { VizTree } from "./model/tree";
import { layoutTree } from "./model/treeLayout";
import { centerOn, type Transform } from "./model/viewport";
import { defaultOptions, encodeOptions, graphArgs, TREE_LABEL_ZOOM, type VizOptions } from "./options";
import { isDirectoryKind, layoutOf, type VizWindowKind } from "./open";
import { Sidebar, type ExportState } from "./Sidebar";

export interface GraphViewProps {
  kind: VizWindowKind;
  snapshotId: string;
  /** Erhöht sich bei „Aktualisieren“ (neuer Schnappschuss, Wurzel und Ansicht bleiben, UI-090). */
  generation: number;
  initialRoot: number | null;
  /** Vorgabe für den CSV-Zielordner außerhalb der App (E2E/Harness). */
  exportDir?: string | null;
}

export type LayoutState = { phase: "idle" | "running" | "paused" | "done"; steps: number; reason?: string; ms?: number };

interface Loaded {
  tree: VizTree;
  dto: GraphDto;
  root: number | null;
}

export interface VizDebug {
  kind: VizWindowKind;
  nodes: number;
  edges: number;
  truncated: boolean;
  layout: LayoutState;
  transform: Transform;
  trail: { id: number | null; label: string }[];
  firstFrameMs: number | null;
  loadedAt: number | null;
  svg(): string;
  csv(): ReturnType<typeof csvPayload> | null;
  positions(): { id: number; x: number; y: number }[];
  screenOf(id: number): [number, number] | null;
  matches: number[];
  setTransform(t: Transform): void;
  panBy(dx: number, dy: number): void;
  /** Klick auf einen Knoten (Baum: ein-/ausklappen). */
  click(id: number): void;
  /** Eingeblendete Links (UI-097): Kantenzahl, Gesamtzahl laut Kern, hervorgehobene Knoten. */
  links: { edges: number; total: number; connected: number[]; mode: string } | null;
}

declare global {
  interface Window {
    __osViz?: VizDebug;
  }
}

export function GraphView({ kind, snapshotId, generation, initialRoot, exportDir }: GraphViewProps) {
  const t = useT();
  const layout = layoutOf(kind) === "force" ? "force" : "tree";
  const [opts, setOpts] = useState<VizOptions>(() => defaultOptions(kind));
  const [links, setLinks] = useState<LinkRequest | null>(null);
  const [overlay, setOverlay] = useState<LinkOverlay | null>(null);
  const [trail, setTrail] = useState(() => RootTrail.start(t("viz-trail-start")));
  const [data, setData] = useState<Loaded | null>(null);
  const [treeVersion, bumpTree] = useReducer((x: number) => x + 1, 0);
  const [frame, bumpFrame] = useReducer((x: number) => x + 1, 0);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [layoutState, setLayoutState] = useState<LayoutState>({ phase: "idle", steps: 0 });
  const [searchState, setSearchState] = useState<SearchState>(EMPTY_SEARCH);
  const [menuTarget, setMenuTarget] = useState<number | null>(null);
  const [confirmFocus, setConfirmFocus] = useState<number | null>(null);
  const [exportState, setExportState] = useState<ExportState>({ kind: "idle" });
  const [csvDir, setCsvDir] = useState(exportDir ?? "");
  const canvas = useRef<GraphCanvasHandle>(null);
  const positions = useRef(new Map<number, [number, number]>());
  const fixed = useRef(new Map<number, [number, number]>());
  const runner = useRef<ForceRunner | null>(null);
  const runIds = useRef<number[]>([]);
  const pendingView = useRef<Transform | "reset" | null>("reset");
  const loadSeq = useRef(0);
  const exportOff = useRef<(() => void) | null>(null);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      loadSeq.current++;
      exportOff.current?.();
      exportOff.current = null;
      delete window.__osViz;
    };
  }, []);
  const timing = useRef({ start: performance.now(), firstFrame: null as number | null, loaded: null as number | null });
  const style = useRenderStyle(opts.lineWidth);
  const enc = useMemo(() => encodeOptions(kind, opts, style.palette), [kind, opts, style.palette]);
  const rootId = trail.current.id ?? initialRoot;

  // ---- Laden ----------------------------------------------------------------------------------------------------
  const dataRef = useRef<Loaded | null>(null);
  dataRef.current = data;
  const applyLoaded = useCallback((dto: GraphDto, root: number | null, keepCollapsed: Set<number> | null, req: LinkRequest | null = null) => {
    // Eingeblendete Links ändern die Struktur nicht: gleicher Baum, kein Neustart des Layouts (UI-097).
    const prev = dataRef.current;
    const keep = keepCollapsed === null && prev !== null && prev.root === root && sameStructure(prev.dto, dto);
    const tree = keep ? prev.tree : VizTree.from({ ...dto, edges: treeEdges(dto) });
    if (keepCollapsed) for (const id of keepCollapsed) if (tree.get(id)) tree.collapsed.add(id);
    timing.current.loaded = performance.now() - timing.current.start;
    setOverlay(req ? overlayFrom(dto, req, tree) : null);
    setData({ tree, dto, root });
    setTrail((tr) => (tr.entries.length === 1 && tr.current.view === null && tree.roots[0] !== undefined && root === (tr.current.id ?? initialRoot)
      ? new RootTrail([{ ...tr.current, label: tree.get(tree.roots[0])?.address ?? tree.get(tree.roots[0])?.label ?? tr.current.label }])
      : tr));
  }, [initialRoot]);

  const linkKey = JSON.stringify(links);
  const optsKey = JSON.stringify(withLinks(graphArgs(kind, snapshotId, rootId, opts), links, opts));
  useEffect(() => {
    const seq = ++loadSeq.current;
    const ctrl = new AbortController();
    const req = JSON.parse(linkKey) as LinkRequest | null;
    setLoading(true);
    setError(null);
    viz
      .graph(JSON.parse(optsKey), ctrl.signal)
      .then((dto) => {
        if (seq !== loadSeq.current) return;
        applyLoaded(dto, rootId, null, req);
      })
      .catch((e: unknown) => {
        if (seq === loadSeq.current && !ctrl.signal.aborted) setError(describeError(t, toCmdError(e)).text);
      })
      .finally(() => seq === loadSeq.current && setLoading(false));
    return () => ctrl.abort();
    // `generation` lädt nach „Aktualisieren“ neu; Wurzel und Ansicht bleiben erhalten.
  }, [optsKey, linkKey, generation, applyLoaded, rootId, t]);

  const explore = useCallback(
    async (id: number) => {
      if (!data) return;
      const seq = ++loadSeq.current;
      setLoading(true);
      try {
        // Eingeblendete Links bleiben erhalten und werden für die erweiterte Knotenmenge neu bestimmt.
        const dto = await viz.graph(withLinks(graphArgs(kind, snapshotId, rootId, opts, id), links, opts));
        if (seq !== loadSeq.current) return;
        const keep = new Set(data.tree.collapsed);
        keep.delete(id);
        applyLoaded(dto, rootId, keep, links);
      } catch (e) {
        if (seq === loadSeq.current) setError(describeError(t, toCmdError(e)).text);
      } finally {
        if (seq === loadSeq.current) setLoading(false);
      }
    },
    [applyLoaded, data, kind, links, opts, rootId, snapshotId, t],
  );

  const focusRoot = useCallback(
    (id: number) => {
      const n = data?.tree.get(id);
      if (!n || !canvas.current) return;
      setTrail((tr) => tr.focus(id, n.address ?? n.label, canvas.current?.getTransform() ?? { k: 1, x: 0, y: 0 }));
      setLinks(null);
      pendingView.current = "reset";
      positions.current.clear();
      fixed.current.clear();
    },
    [data],
  );

  const backTo = useCallback((index: number) => {
    setLinks(null);
    setTrail((tr) => {
      const { trail: next, view } = tr.back(index);
      pendingView.current = view ?? "reset";
      positions.current.clear();
      fixed.current.clear();
      return next;
    });
  }, []);

  // ---- Layout ---------------------------------------------------------------------------------------------------
  const tree = data?.tree ?? null;
  const treePositions = useMemo(() => {
    if (!tree || layout !== "tree") return null;
    return layoutTree(tree, { orientation: opts.orientation, levelGap: opts.levelGap, nodeGap: opts.nodeGap });
    // treeVersion: Ein-/Ausklappen ändert die sichtbare Menge.
  }, [tree, treeVersion, layout, opts.orientation, opts.levelGap, opts.nodeGap]);

  const scene: Scene | null = useMemo(() => {
    if (!tree) return null;
    return buildScene(tree, treePositions ?? positions.current, enc, opts.labelMode);
  }, [tree, treeVersion, treePositions, enc, opts.labelMode]);
  const overlayScene = useMemo(() => (scene ? sceneOverlay(scene, overlay) : null), [scene, overlay]);

  // Kraftgerichtetes Layout im Worker; Neustart bei neuer Knotenmenge, anderen Radien oder anderen Kräften. Zwischenstände
  // gehen an die jeweils aktuelle Szene (Farbwechsel baut die Szene neu, ohne das Layout neu zu starten).
  const sceneRef = useRef<Scene | null>(null);
  sceneRef.current = scene;
  const radiusKey = useMemo(() => scene && layout === "force" ? scene.nodes.map((n) => n.r).join(",") : "", [scene, layout]);
  useEffect(() => {
    if (layout !== "force" || !scene || !tree) return;
    const ids = scene.nodes.map((n) => n.id);
    const at = new Map(ids.map((id, i) => [id, i]));
    const parent = new Int32Array(ids.length);
    const radius = new Float64Array(ids.length);
    const init = new Float64Array(ids.length * 2).fill(Number.NaN);
    const fix = new Float64Array(ids.length * 2).fill(Number.NaN);
    ids.forEach((id, i) => {
      const n = tree.get(id);
      parent[i] = n?.parent != null ? (at.get(n.parent) ?? -1) : -1;
      radius[i] = (scene.nodes[i] as Scene["nodes"][number]).r;
      const p = positions.current.get(id);
      if (p) {
        init[i * 2] = p[0];
        init[i * 2 + 1] = p[1];
      }
      const f = fixed.current.get(id);
      if (f) {
        fix[i * 2] = f[0];
        fix[i * 2 + 1] = f[1];
      }
    });
    const input: ForceInput = { parent, radius, init, fixed: fix, params: { repulsion: opts.repulsion, springLength: opts.springLength, seed: opts.seed, allowOverlap: opts.allowOverlap } };
    runIds.current = ids;
    runner.current?.dispose();
    const onReply = (r: ForceReply) => {
      if (runIds.current !== ids) return;
      if (sceneRef.current) applyPositions(sceneRef.current, ids, r.positions);
      ids.forEach((id, i) => {
        const p = positions.current.get(id);
        const x = r.positions[i * 2] as number, y = r.positions[i * 2 + 1] as number;
        if (p) { p[0] = x; p[1] = y; }
        else positions.current.set(id, [x, y]);
      });
      canvas.current?.redraw();
      bumpFrame();
      if (r.type === "done") setLayoutState({ phase: "done", steps: r.steps, reason: r.reason, ms: performance.now() - startedAt });
      else setLayoutState((s) => (s.phase === "paused" ? s : { phase: "running", steps: r.steps }));
    };
    const startedAt = performance.now();
    const run = createForceRunner(onReply);
    runner.current = run;
    setLayoutState({ phase: "running", steps: 0 });
    run.start(input);
    return () => {
      run.dispose();
      if (runner.current === run) runner.current = null;
    };
  }, [layout, tree, treeVersion, radiusKey, opts.repulsion, opts.springLength, opts.seed, opts.allowOverlap]);

  // Ein neuer Startwert beginnt mit frischer Anordnung (bitgleiches Ergebnis je Startwert, UI-092).
  const lastSeed = useRef(opts.seed);
  if (lastSeed.current !== opts.seed) {
    lastSeed.current = opts.seed;
    positions.current.clear();
  }

  // Ansicht nach dem Laden: neue Wurzel → Startansicht; Rückkehr über die Pfadleiste → gemerkte Ansicht.
  useEffect(() => {
    if (!scene || !canvas.current) return;
    const v = pendingView.current;
    pendingView.current = null;
    if (v === "reset") canvas.current.resetView();
    else if (v) canvas.current.setTransform(v);
  }, [scene]);

  const initialTransform = useCallback(
    (w: number, h: number): Transform => {
      if (layout === "force") return { k: 1, x: w / 2, y: h / 2 };
      return opts.orientation === "lr" ? { k: 1, x: 48, y: h / 2 } : { k: 1, x: w / 2, y: 48 };
    },
    [layout, opts.orientation],
  );

  // ---- Suche ----------------------------------------------------------------------------------------------------
  useEffect(() => {
    if (!tree) return;
    setSearchState((s) => (s.query ? runSearch(tree, s.query) : s));
  }, [tree]);
  const currentHit = currentMatch(searchState);
  const highlights = useMemo(() => new Set(searchState.matches), [searchState.matches]);
  useEffect(() => {
    if (currentHit === null || !tree) return;
    // Treffer in eingeklappten Teilen: Pfad automatisch öffnen (UI-090).
    if (tree.reveal(currentHit)) bumpTree();
  }, [currentHit, tree]);
  // Nur bei einem neuen aktuellen Treffer zentrieren – nicht bei jedem Neuladen (Rückkehr stellt die gemerkte Ansicht her).
  const centered = useRef<number | null>(null);
  useEffect(() => {
    if (currentHit === null) centered.current = null;
    if (currentHit === null || currentHit === centered.current || !scene || !canvas.current) return;
    const k = scene.index.get(currentHit);
    if (k === undefined) return;
    centered.current = currentHit;
    const n = scene.nodes[k] as Scene["nodes"][number];
    const { width, height } = canvas.current.size();
    canvas.current.setTransform(centerOn(canvas.current.getTransform(), n.x, n.y, width, height));
  }, [currentHit, scene]);

  // ---- Interaktion ----------------------------------------------------------------------------------------------
  const requestExplore = useCallback(
    (id: number) => {
      if (!tree) return;
      if (tree.collapsed.has(id)) {
        tree.toggle(id);
        bumpTree();
        return;
      }
      const n = tree.get(id);
      const decision = exploreDecision(n?.hiddenChildren ?? 0, tree.size, opts.budget);
      if (decision === "confirm") setConfirmFocus(id);
      else if (decision === "explore") void explore(id);
    },
    [explore, opts.budget, tree],
  );

  const onNodeClick = useCallback(
    (id: number) => {
      if (!tree || layout !== "tree") return;
      const r = tree.toggle(id);
      if (r === "explore") requestExplore(id);
      else if (r !== "none") bumpTree();
    },
    [layout, requestExplore, tree],
  );

  const onNodeDrag = useCallback(
    (id: number, x: number, y: number) => {
      if (!scene) return;
      const k = scene.index.get(id);
      if (k === undefined) return;
      const n = scene.nodes[k] as Scene["nodes"][number];
      n.x = x;
      n.y = y;
      positions.current.set(id, [x, y]);
      fixed.current.set(id, [x, y]);
      const i = runIds.current.indexOf(id);
      if (i >= 0) runner.current?.fix(i, x, y);
    },
    [scene],
  );

  const release = useCallback((id: number) => {
    if (!fixed.current.delete(id)) return;
    const i = runIds.current.indexOf(id);
    if (i >= 0) runner.current?.release(i);
  }, []);

  const fmt = getFormatters();
  const tooltip = useCallback(
    (id: number) => {
      const n = tree?.get(id);
      if (!n || !tree) return null;
      const rows = tooltipRows(n, directoryPath(tree, id), opts.metric, (v) => (Number.isInteger(v) ? fmt.integer(v) : fmt.decimal(v, 2)));
      const hidden = tree.isCapped(id) ? tree.hidden(id) : null;
      return (
        <dl className="os-viz-tip">
          {rows.map((r, i) => (
            <div key={i} className="os-viz-tip-row">
              <dt>{r.name}</dt>
              <dd>{r.value}</dd>
            </div>
          ))}
          {hidden && <p className="os-viz-tip-hidden">{t("viz-hidden-children", { count: hidden.children, total: hidden.descendants })}</p>}
        </dl>
      );
    },
    [fmt, opts.metric, t, tree],
  );

  const menuEntries: MenuEntry[] = useMemo(() => {
    const fit: MenuEntry = { id: "viz-fit", label: t("viz-menu-reset-view"), onAction: () => canvas.current?.resetView() };
    const n = menuTarget !== null ? tree?.get(menuTarget) : undefined;
    if (!n || !tree) return [fit];
    const isRoot = tree.roots.includes(n.id);
    return [
      {
        id: "viz-show-in-table",
        label: t("viz-menu-show-in-table"),
        isDisabled: n.address == null,
        disabledReason: n.address == null ? t("viz-reason-path-node") : undefined,
        onAction: () => void viz.focusInMain({ urlId: n.id }).catch((e: unknown) => setError(describeError(t, toCmdError(e)).text)),
      },
      {
        id: "viz-explore",
        label: t("viz-menu-explore"),
        isDisabled: !tree.isCapped(n.id),
        disabledReason: tree.isCapped(n.id) ? undefined : t("viz-reason-nothing-hidden"),
        onAction: () => requestExplore(n.id),
      },
      {
        id: "viz-focus",
        label: t("viz-menu-focus"),
        isDisabled: isRoot && tree.roots.length === 1,
        onAction: () => focusRoot(n.id),
      },
      ...(layout === "force" && fixed.current.has(n.id) ? [{ id: "viz-release", label: t("viz-menu-release"), onAction: () => release(n.id) }] : []),
      ...(layout === "force" ? linkMenuEntries(t, n, overlay, setLinks, () => setLinks(null)) : []),
      { ...fit, separatorBefore: true },
    ];
  }, [focusRoot, layout, menuTarget, overlay, release, requestExplore, t, tree]);

  // ---- Exporte --------------------------------------------------------------------------------------------------
  const exportSvg = useCallback(() => {
    const svg = canvas.current?.svg();
    if (svg) saveBlob(`${kind}.svg`, svgBlob(svg));
  }, [kind]);

  const exportPng = useCallback(
    async (factor: 1 | 2) => {
      const blob = await canvas.current?.png(factor);
      if (blob) saveBlob(`${kind}@${factor}x.png`, blob);
    },
    [kind],
  );

  const csvBlocked = !!tree && tree.hasClientCollapse();
  const exportCsv = useCallback(async () => {
    if (!tree || !scene || csvBlocked) return;
    try {
      const dir = isTauri() ? await chooseFolder(t("viz-export-csv-choose")) : csvDir.trim();
      if (!dir) return;
      const payload = csvPayload(tree, scene, overlayCsvEdges(overlay));
      setExportState({ kind: "running" });
      const handle = await viz.exportCsv({ snapshotId, dir, nodes: payload.nodes, edges: payload.edges, layout: kind });
      if (!mounted.current) return;
      exportOff.current?.();
      const off = eventBus.on("tick", (tick) => {
        const op: OpStatus | undefined = tick.ops.find((o) => o.opId === handle.opId);
        if (!op || op.state === "running") return;
        off();
        exportOff.current = null;
        if (op.state === "done") setExportState({ kind: "done", path: dir });
        else setExportState({ kind: "failed", message: op.error ? describeError(t, op.error).text : t("viz-export-failed") });
      });
      exportOff.current = off;
    } catch (e) {
      setExportState({ kind: "failed", message: describeError(t, toCmdError(e)).text });
    }
  }, [csvBlocked, csvDir, kind, overlay, scene, snapshotId, t, tree]);

  // ---- Testzugang (nur Entwicklungs-/E2E-Build) -------------------------------------------------------------------
  useEffect(() => {
    if (!import.meta.env.DEV && import.meta.env.VITE_E2E !== "1") return;
    window.__osViz = {
      kind,
      nodes: scene?.nodes.length ?? 0,
      edges: scene?.edges.length ?? 0,
      truncated: data?.dto.truncated ?? false,
      layout: layoutState,
      get transform() {
        return canvas.current?.getTransform() ?? { k: 1, x: 0, y: 0 };
      },
      trail: trail.entries.map((e) => ({ id: e.id, label: e.label })),
      get firstFrameMs() {
        return timing.current.firstFrame;
      },
      get loadedAt() {
        return timing.current.loaded;
      },
      svg: () => canvas.current?.svg() ?? "",
      csv: () => (tree && scene ? csvPayload(tree, scene, overlayCsvEdges(overlay)) : null),
      positions: () => scene?.nodes.map((n) => ({ id: n.id, x: n.x, y: n.y })) ?? [],
      screenOf: (id) => {
        const k = scene?.index.get(id);
        const tr = canvas.current?.getTransform();
        if (k === undefined || !scene || !tr) return null;
        const n = scene.nodes[k] as Scene["nodes"][number];
        return [n.x * tr.k + tr.x, n.y * tr.k + tr.y];
      },
      matches: searchState.matches,
      setTransform: (tr) => canvas.current?.setTransform(tr),
      panBy: (dx, dy) => {
        const tr = canvas.current?.getTransform();
        if (tr) canvas.current?.setTransform({ k: tr.k, x: tr.x + dx, y: tr.y + dy });
      },
      click: (id) => onNodeClick(id),
      links: overlay ? { edges: overlayScene?.edges.length ?? 0, total: overlay.total, connected: [...overlay.connected], mode: overlay.request.mode } : null,
    };
  });

  const labelsAt = useCallback((k: number) => k >= (layout === "force" ? opts.labelZoom : TREE_LABEL_ZOOM), [layout, opts.labelZoom]);

  return (
    <div className="os-viz-body">
      <Sidebar
        kind={kind}
        options={opts}
        onOptions={(patch) => setOpts((o) => ({ ...o, ...patch }))}
        availableMetrics={data?.dto.availableMetrics ?? []}
        stats={scene?.stats ?? null}
        scene={scene}
        search={searchState}
        onSearch={(q) => setSearchState(tree ? runSearch(tree, q) : { ...EMPTY_SEARCH, query: q })}
        onStep={(dir) => setSearchState((s) => stepSearch(s, dir))}
        onExpandDepth={() => {
          tree?.expandAll();
          bumpTree();
        }}
        layoutState={layoutState}
        onPause={() => {
          runner.current?.pause();
          setLayoutState((s) => ({ ...s, phase: "paused" }));
        }}
        onResume={() => {
          runner.current?.resume();
          setLayoutState((s) => ({ ...s, phase: "running" }));
        }}
        exportState={exportState}
        onExportSvg={exportSvg}
        onExportPng={(f) => void exportPng(f)}
        onExportCsv={() => void exportCsv()}
        csvBlocked={csvBlocked}
        csvDir={isTauri() ? null : csvDir}
        onCsvDir={setCsvDir}
        hasData={!!scene && scene.nodes.length > 0}
        links={layout === "force" ? { overlay, shown: overlayScene?.edges.length ?? 0, onReset: () => setLinks(null) } : null}
      />
      <section className="os-viz-main" aria-label={t(`viz-kind-${kind}`)}>
        <nav className="os-viz-trail" aria-label={t("viz-trail-label")}>
          <ol>
            {trail.entries.map((e, i) => (
              <li key={`${e.id ?? "start"}-${i}`}>
                {i < trail.entries.length - 1 ? (
                  <Button variant="tool" onPress={() => backTo(i)}>
                    {e.label}
                  </Button>
                ) : (
                  <span aria-current="page">{e.label}</span>
                )}
              </li>
            ))}
          </ol>
          {loading && <span className="os-viz-loading" role="status">{t("viz-loading")}</span>}
          {data?.dto.truncated && <span className="os-viz-truncated">{t("viz-truncated", { count: scene?.nodes.length ?? 0 })}</span>}
          <span className="os-viz-count" data-testid="viz-node-count">{t("viz-node-count", { count: scene?.nodes.length ?? 0 })}</span>
        </nav>
        {error && (
          <Banner kind="error" onDismiss={() => setError(null)} dismissLabel={t("ui-dialog-close")}>
            {error}
          </Banner>
        )}
        {scene && (
          <ContextMenu entries={menuEntries} label={t("viz-menu-label")} className="os-viz-stage-host">
            <GraphCanvas
              ref={canvas}
              scene={scene}
              version={frame + treeVersion}
              style={style}
              labelsAt={labelsAt}
              highlights={highlights}
              current={currentHit}
              overlay={overlayScene}
              draggable={layout === "force"}
              label={t("viz-canvas-label", { kind: t(`viz-kind-${kind}`), count: scene.nodes.length })}
              initialTransform={initialTransform}
              onNodeClick={onNodeClick}
              onNodeDoubleClick={layout === "force" ? release : undefined}
              onNodeDrag={onNodeDrag}
              onContextTarget={setMenuTarget}
              onKeyTarget={() => setMenuTarget(currentHit)}
              onFirstFrame={() => {
                timing.current.firstFrame = performance.now() - timing.current.start;
                performance.mark?.("viz-first-frame");
              }}
              tooltip={tooltip}
            />
            <CompactLegend
              colorMode={opts.colorMode}
              colors={enc.colors && opts.palette === "standard" ? enc.colors : PALETTES[opts.palette]}
              segments={opts.colorMode === "segment" ? [...new Set(scene.nodes.filter((n) => !n.capped && !n.hollow).map((n) => n.fill))] : []}
              directory={isDirectoryKind(kind)}
              links={overlay ? { inlink: overlay.edges.some((e) => e.type === "inlink"), outlink: overlay.edges.some((e) => e.type === "outlink"), inColor: style.inlink, outColor: style.outlink } : null}
            />
          </ContextMenu>
        )}
      </section>
      <Dialog
        title={t("viz-confirm-title")}
        isOpen={confirmFocus !== null}
        onOpenChange={(o) => !o && setConfirmFocus(null)}
        footer={(close) => (
          <DialogFooter
            okLabel={t("viz-menu-focus")}
            cancelLabel={t("ui-dialog-cancel")}
            onOk={() => {
              if (confirmFocus !== null) focusRoot(confirmFocus);
              close();
            }}
            onCancel={close}
          />
        )}
      >
        <p>{t("viz-confirm-body", { count: (confirmFocus !== null ? tree?.get(confirmFocus)?.hiddenChildren : 0) ?? 0 })}</p>
      </Dialog>
    </div>
  );
}
