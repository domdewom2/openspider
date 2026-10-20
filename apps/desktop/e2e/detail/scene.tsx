import { createRoot } from "react-dom/client";
import { useState } from "react";
import { DetailFrame } from "../../src/features/detail/frame/DetailFrame";
import { detailFixtureCore } from "../../src/features/detail/frame/fixtures/core";
import type { DetailSelection, NavigationRequest } from "../../src/features/detail/frame/types";
import { createClient, setClient, client } from "../../src/ipc/client";
import { createBridgeTransport } from "../../src/ipc/transport/bridge";
import { applyAppearance, DEFAULT_APPEARANCE } from "../../src/ui/tokens/theme";
import { setLanguage } from "../../src/i18n";
import "../../src/ui/ui.css";

const params = new URLSearchParams(location.search);
const fixture = detailFixtureCore(); fixture.options.stored = params.get("stored") !== "false";
if (params.has("large")) fixture.options.total = 1_000_000;
const bridge = params.get("bridge");
setClient(createClient(bridge ? createBridgeTransport({ url: bridge }) : fixture.core.transport()));
applyAppearance(document.documentElement, DEFAULT_APPEARANCE, { platform: "mac", systemDark: false });
Object.assign(window, { detailFixture: fixture, detailInvoke: client.invoke.bind(client) });
function Scene() {
  const [selection, setSelection] = useState<DetailSelection>({ urlIds: [], crawlId: "fixture" });
  const [navigation, setNavigation] = useState<NavigationRequest>();
  return <div style={{ height: "100vh", display: "flex", flexDirection: "column" }}>
    <div><button onClick={() => setSelection({ urlIds: [1], focus: 1, topTab: "Internal", crawlId: "fixture" })}>Select one URL</button>
      <button onClick={() => setSelection({ urlIds: [1, 2, 3], focus: 2, topTab: "Response Codes", crawlId: "fixture" })}>Select three URLs</button>
      <button onClick={() => setSelection({ urlIds: [7], focus: 7, topTab: "Images", crawlId: "fixture" })}>Select image</button>
      <button onClick={() => setSelection({ urlIds: [], crawlId: "fixture" })}>Clear selection</button></div>
    <output aria-label="Navigation">{navigation ? JSON.stringify(navigation) : ""}</output>
    <DetailFrame selection={selection} actions={{ navigate: setNavigation, openBrowser: address => setNavigation({ address, tab: "auto" }) }} />
  </div>;
}
void setLanguage(params.get("lang") === "de" ? "de" : "en").then(() => createRoot(document.getElementById("root")!).render(<Scene />));
