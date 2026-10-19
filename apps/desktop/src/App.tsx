import { ExportHost, ExportViewButton } from "./features/export";
import { McpHost, McpStatusField } from "./features/mcp";
import { useEffect, useState, type ComponentType } from "react";
import { RegionBoundary } from "./app/ErrorBoundary";
import { Providers } from "./app/Providers";
import { useT } from "./i18n";
import { app as appApi } from "./ipc/app";
import { EmptyState } from "./ui/primitives/EmptyState";

/** Optional main-window regions; absent registrations retain the scaffold defaults. */
const mainRegions: { tabs?: ComponentType; table?: ComponentType; detail?: ComponentType } = {};
export function registerMainRegions(regions: Partial<typeof mainRegions>): void {
  Object.assign(mainRegions, regions);
}

/**
 * Hauptfenster (Gerüst, M2-F04): Landmarks nach M2-ui-concept 1.2 mit leeren Regionen. Die Fachpakete (U01 Layout,
 * U03 Kopf-/Statusleiste, U10 Tabelle, U11 Details, U12 Seitenleiste) füllen sie.
 */
export function App() {
  const TabsRegion = mainRegions.tabs;
  const TableRegion = mainRegions.table;
  const DetailRegion = mainRegions.detail;
  const t = useT();
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    document.title = t("app-title");
  }, [t]);

  useEffect(() => {
    let live = true;
    appApi
      .info()
      .then((info) => live && setVersion(info.version))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);

  return (
    <Providers>
      <ExportHost />
      <McpHost />
      <div className="os-app">
        <header className="os-app-header" role="banner" aria-label={t("app-region-header")}>
          <h1 className="os-app-title">{t("app-title")}</h1>
        </header>
        <nav className="os-app-tabs" aria-label={t("app-region-tabs")}>{TabsRegion && <TabsRegion />}</nav>
        <main className="os-app-main" data-region="table" aria-label={t("app-region-table")}>
          <ExportViewButton />
          <RegionBoundary name="table">
            {TableRegion ? <TableRegion /> : <EmptyState state={{ code: "NO_DATA_YET" }} />}
          </RegionBoundary>
          {DetailRegion && <RegionBoundary name="detail"><DetailRegion /></RegionBoundary>}
        </main>
        <aside className="os-app-sidebar" aria-label={t("app-region-sidebar")} />
        <footer className="os-app-status" role="contentinfo" aria-label={t("app-region-status")}>
          <McpStatusField />
          {version !== null && <span data-testid="version">{t("app-version", { version })}</span>}
        </footer>
      </div>
    </Providers>
  );
}
