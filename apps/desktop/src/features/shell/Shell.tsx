import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { RegionBoundary } from "../../app/ErrorBoundary";
import { mainRegions } from "../../app/mainRegions";
import { useT } from "../../i18n";
import { toCmdError } from "../../ipc/errors";
import { useLayout, persistLayout } from "../../state/layout";
import { registerAction } from "../../ui/actions";
import { Button } from "../../ui/primitives/Button";
import { Dialog, DialogFooter } from "../../ui/primitives/Dialog";
import { Divider } from "./Divider";
import { defaultLayout, finishDrag, measureLayout, resetDivider, type Pane, type Viewport } from "./layout";
import { QuitDialog } from "./QuitDialog";
import { bindWindowGeometry } from "./window";
import "./shell.css";

/** Additive shell controller: existing header/table/sidebar/status regions remain feature-owned. */
export function Shell() {
  const t = useT();
  const DetailRegion = mainRegions.detail;
  const anchor = useRef<HTMLSpanElement>(null);
  const [viewport, setViewport] = useState<Viewport>({ width: 0, height: 0 });
  const [preview, setPreview] = useState<Partial<Record<Pane, number>>>({});
  const [resetOpen, setResetOpen] = useState(false);
  const { layout, loaded, error } = useLayout();
  const [geometryReady, setGeometryReady] = useState(false);
  const space = viewport.width > 0 && viewport.height > 0 ? viewport : { width: 1440, height: 836 };
  const effective = loaded ? layout : defaultLayout(space.width, space.height);
  const pixels = measureLayout(space, effective);
  for (const pane of ["sidebar", "detail"] as const) {
    if (preview[pane] !== undefined) pixels[pane] = preview[pane];
  }
  useLayoutEffect(() => {
    const root = anchor.current?.parentElement;
    if (!root) return;
    root.classList.add("os-shell");
    const measure = () => {
      const header = root.querySelector<HTMLElement>(":scope > header");
      const footer = root.querySelector<HTMLElement>(":scope > footer");
      // Fractional heights (header/status bar content from other packages) must not be rounded to whole pixels.
      setViewport({ width: root.clientWidth, height: root.clientHeight - (header?.getBoundingClientRect().height ?? 40) - (footer?.getBoundingClientRect().height ?? 24) });
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(root);
    const header = root.querySelector("header");
    const footer = root.querySelector("footer");
    if (header) observer.observe(header);
    if (footer) observer.observe(footer);
    return () => { observer.disconnect(); root.classList.remove("os-shell"); };
  }, []);
  useEffect(() => {
    if (viewport.width <= 0 || viewport.height <= 0) return;
    void useLayout.getState().load(viewport.width, viewport.height).catch(() => undefined);
  }, [viewport.width, viewport.height]);
  useEffect(() => persistLayout(), []);
  useEffect(() => {
    if (!loaded) return;
    let live = true;
    let off: (() => void) | undefined;
    void bindWindowGeometry().then((dispose) => {
      if (live) { off = dispose; setGeometryReady(true); }
      else dispose();
    }).catch((e) => { if (live) useLayout.setState({ error: toCmdError(e) }); });
    return () => { live = false; off?.(); };
  }, [loaded]);
  useLayoutEffect(() => {
    const root = anchor.current?.parentElement;
    if (!root) return;
    root.style.setProperty("--shell-sidebar", `${pixels.sidebar}px`);
    root.style.setProperty("--shell-detail", `${pixels.detail}px`);
    root.style.setProperty("--shell-sidebar-grip", `${pixels.sidebarGrip}px`);
    root.style.setProperty("--shell-detail-grip", `${pixels.detailGrip}px`);
    const sidebar = root.querySelector<HTMLElement>(":scope > aside");
    if (sidebar) {
      sidebar.id = "shell-sidebar";
      sidebar.hidden = pixels.sidebar === 0;
    }
  }, [pixels.sidebar, pixels.detail, pixels.sidebarGrip, pixels.detailGrip]);
  useEffect(() => {
    const toggle = (pane: Pane) => {
      const s = useLayout.getState();
      if (!s.loaded) return;
      s.setLayout({ ...s.layout, [pane]: { ...s.layout[pane], visible: !(s.layout[pane].visible && !s.layout[pane].collapsed), collapsed: false } });
    };
    const offs = [registerAction("view.toggleDetail", () => toggle("detail")),
      registerAction("view.toggleSidebar", () => toggle("sidebar")),
      registerAction("view.resetColumns", () => {
        if (!useLayout.getState().loaded) return false;
        setResetOpen(true);
      })];
    return () => offs.forEach((off) => off());
  }, []);
  const commit = (pane: Pane, size: number) => {
    const store = useLayout.getState();
    if (!store.loaded) return;
    store.setLayout(finishDrag(store.layout, pane, size, viewport));
    void store.flush().catch(() => undefined);
  };
  return <>
    <span hidden ref={anchor} data-shell-ready={loaded && geometryReady ? "true" : "false"} />
    {(["sidebar", "detail"] as const).map((pane) => effective[pane].visible && <Divider key={pane} pane={pane}
      label={t(`shell-divider-${pane}`)} collapsed={effective[pane].collapsed} size={pixels[pane]} viewport={viewport}
      onCommit={(size) => commit(pane, size)}
      onPreview={(size) => setPreview((old) => ({ ...old, [pane]: size ?? undefined }))}
      onRestore={() => { const s = useLayout.getState(); if (!s.loaded) return; s.setLayout({ ...s.layout, [pane]: { ...s.layout[pane], collapsed: false } }); }}
      onReset={() => { const s = useLayout.getState(); if (!s.loaded) return; s.setLayout(resetDivider(s.layout, pane, viewport)); }} />)}
    {/* Eine registrierte Detailregion (U11) bringt ihren eigenen Landmark mit; ohne sie gilt der Platzhalter. */}
    <section id="shell-detail" className="os-shell-detail" hidden={pixels.detail === 0}
      {...(DetailRegion ? {} : { role: "region", "aria-label": t("shell-details") })}>
      {DetailRegion
        ? <RegionBoundary name="detail"><DetailRegion /></RegionBoundary>
        : <div className="os-shell-detail-placeholder">{t("shell-detail-empty")}</div>}
    </section>
    {error && <div className="os-shell-error" role="alert">{t("shell-state-error")} <Button onPress={() => {
      const s = useLayout.getState();
      void (s.loaded ? s.flush() : s.load(viewport.width, viewport.height)).catch(() => undefined);
    }}>{t("shell-retry")}</Button></div>}
    <Dialog title={t("shell-reset-columns")} isOpen={resetOpen} onOpenChange={setResetOpen}
      footer={(close) => <DialogFooter okLabel={t("ui-dialog-reset")} cancelLabel={t("ui-dialog-cancel")}
        onCancel={close} onOk={() => { useLayout.getState().resetColumns(); close(); }} />}>
      <p>{t("shell-reset-description")}</p>
    </Dialog>
    <QuitDialog />
  </>;
}
