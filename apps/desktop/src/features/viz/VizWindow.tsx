// Visualisierungsfenster (UI-090): nicht modal, Schnappschuss des Crawl-Stands beim Öffnen. Ändern sich die Daten,
// meldet der Kern `viz-data-changed`; die Kopfzeile zeigt dann „Daten geändert – Aktualisieren“, erst ein Klick lädt
// neu (Wurzel und Zoom bleiben erhalten).
import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { describeError, toCmdError } from "../../ipc/errors";
import { eventBus } from "../../ipc/events";
import type { Notice } from "../../ipc/gen/Notice";
import { viz } from "../../ipc/viz";
import { Banner, Button, EmptyState, Select } from "../../ui/primitives";
import { GraphView } from "./GraphView";
import { coreKind, GRAPH_KINDS, isWindowKind, waitForKind, type VizWindowKind, type VizWindowParams } from "./open";
import { WordCloudView } from "./WordCloudView";
import "./viz.css";

export function VizWindow({ params, exportDir }: { params: VizWindowParams; exportDir?: string | null }) {
  const t = useT();
  const [kind, setKind] = useState<VizWindowKind | null>(params.kind);
  const [waiting, setWaiting] = useState(params.kind === null && params.snapshotId !== null);
  const [changed, setChanged] = useState(false);
  const [provenance, setProvenance] = useState(false);
  const [generation, setGeneration] = useState(0);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const snapshotId = params.snapshotId;

  // Die App-Hülle übergibt nur die Schnappschuss-Kennung; die Art schreibt das öffnende Fenster in den gemeinsamen Speicher.
  useEffect(() => {
    if (kind !== null || !snapshotId) return;
    let live = true;
    void waitForKind(snapshotId).then((k) => {
      if (!live) return;
      setWaiting(false);
      if (k) setKind(k);
    });
    return () => {
      live = false;
    };
  }, [kind, snapshotId]);

  useEffect(() => {
    if (kind) document.title = `${t(`viz-kind-${kind}`)} – ${t("app-title")}`;
  }, [kind, t]);

  useEffect(() => {
    const handle = (n: Notice) => {
      if (n.args.snapshotId !== snapshotId) return;
      if (n.code === "viz-data-changed") setChanged(true);
      if (n.code === "viz-provenance-missing") setProvenance(true);
    };
    // Meldungen kommen einzeln oder gebündelt im Tick.
    const offs = [eventBus.on("notice", handle), eventBus.on("tick", (tick) => tick.notices.forEach(handle))];
    return () => offs.forEach((off) => off());
  }, [snapshotId]);

  const refresh = async () => {
    if (!snapshotId || !kind) return;
    setRefreshing(true);
    try {
      await viz.open({ kind: coreKind(kind), rootUrlId: kind === "anchorWords" ? params.rootUrlId : null, refreshSnapshotId: snapshotId });
      setChanged(false);
      setGeneration((g) => g + 1);
    } catch (e) {
      setError(describeError(t, toCmdError(e)).text);
    } finally {
      setRefreshing(false);
    }
  };

  if (!snapshotId) return <EmptyState state={{ code: "NO_DATA_YET" }} />;

  return (
    <div className="os-viz">
      <header className="os-viz-header" role="banner">
        <h1 className="os-viz-title">{kind ? t(`viz-kind-${kind}`) : t("app-viz-title")}</h1>
        {changed && (
          <div className="os-viz-changed" role="status" data-testid="viz-data-changed">
            <span>{t("viz-data-changed")}</span>
            <Button variant="primary" isDisabled={refreshing} onPress={() => void refresh()}>
              {t("viz-refresh")}
            </Button>
          </div>
        )}
      </header>
      {provenance && (
        <Banner kind="info" onDismiss={() => setProvenance(false)} dismissLabel={t("ui-dialog-close")}>
          {t("viz-provenance-missing")}
        </Banner>
      )}
      {error && (
        <Banner kind="error" onDismiss={() => setError(null)} dismissLabel={t("ui-dialog-close")}>
          {error}
        </Banner>
      )}
      {!kind && !waiting && (
        <div className="os-viz-pick">
          <Select
            label={t("viz-pick-kind")}
            value=""
            onChange={(v) => isWindowKind(v) && setKind(v)}
            options={GRAPH_KINDS.map((k) => ({ value: k, label: t(`viz-kind-${k}`) }))}
          />
        </div>
      )}
      {kind === "anchorWords" && <WordCloudView snapshotId={snapshotId} urlId={params.rootUrlId} generation={generation} />}
      {kind && kind !== "anchorWords" && <GraphView kind={kind} snapshotId={snapshotId} generation={generation} initialRoot={params.rootUrlId} exportDir={exportDir} />}
    </div>
  );
}
