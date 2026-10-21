// Prüfaufbau der Fach-Detailtabs (E2E-A, M2-U21): Detailrahmen mit dem Mock-Kern, weil die Kern-Commands von M2-K09/D20/D21 noch nicht gemergt sind.
import { createRoot } from "react-dom/client";
import { useState } from "react";
import { DetailFrame } from "../../src/features/detail/frame/DetailFrame";
import { fachFixtureCore } from "../../src/features/detail/rendered/fixtures/core";
import { CRAWL_ID } from "../../src/features/detail/rendered/fixtures/data";
import type { DetailSelection, NavigationRequest } from "../../src/features/detail/frame/types";
import { createClient, setClient, client } from "../../src/ipc/client";
import { createBridgeTransport } from "../../src/ipc/transport/bridge";
import { installShortcutDispatcher } from "../../src/ui/dispatcher";
import { useExportDialog } from "../../src/features/export/model";
import { applyAppearance, DEFAULT_APPEARANCE } from "../../src/ui/tokens/theme";
import { setLanguage } from "../../src/i18n";
import "../../src/ui/ui.css";

const params = new URLSearchParams(location.search);
const fixture = fachFixtureCore({ largeLines: Number(params.get("large") ?? 0), diffDelay: Number(params.get("diffDelay") ?? 0) });
if (params.has("cfg")) fixture.options.config = JSON.parse(params.get("cfg")!);
// Browser kennen das Schema `ospider://` nicht: Die Kacheln kommen in E2E über einen abfangbaren HTTP-Pfad.
if (params.has("shots")) fixture.core.handlers.set("detail.screenshot", () => ({ url: "/e2e/detail-fach/shot", width: 1024, height: 1536, tiles: 3 }));
const bridge = params.get("bridge");
setClient(createClient(bridge ? createBridgeTransport({ url: bridge }) : fixture.core.transport()));
applyAppearance(document.documentElement, DEFAULT_APPEARANCE, { platform: "mac", systemDark: params.get("dark") === "1" });
installShortcutDispatcher({ platform: "win" });
Object.assign(window, { fach: fixture, fachInvoke: client.invoke.bind(client), exportDialog: useExportDialog });
function Scene() {
  const url = Number(params.get("url") ?? 1);
  const [selection] = useState<DetailSelection>({ urlIds: [url], focus: url, topTab: params.get("top") ?? "Internal", crawlId: CRAWL_ID });
  const [navigation, setNavigation] = useState<NavigationRequest>();
  return <div style={{ height: "100vh", display: "flex", flexDirection: "column" }}>
    <output aria-label="Navigation">{navigation ? JSON.stringify(navigation) : ""}</output>
    <DetailFrame selection={selection} actions={{ navigate: setNavigation, openBrowser: address => setNavigation({ address, tab: "auto" }) }} />
  </div>;
}
void setLanguage(params.get("lang") === "de" ? "de" : "en").then(() => createRoot(document.getElementById("root")!).render(<Scene />));
