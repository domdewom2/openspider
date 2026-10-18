// Contract fixture page for U10 browser tests; never imported by the application entry point.
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App, registerMainRegions } from "../../../App";
import { bootWindow } from "../../../app/runtime";
import { TopTabs } from "../../toptabs/TopTabs";
import { UpperTable } from "../UpperTable";
import { tableFixture } from "./core";
import "../../../ui/ui.css";
import "../../../app/app.css";
import type { UiStateJson } from "../../../ipc/gen/UiStateJson";
const fixture = tableFixture();
const persisted = localStorage.getItem("u10.fixture.uiState");
if (persisted) fixture.core.uiState = JSON.parse(persisted) as UiStateJson;
const savedConfig = localStorage.getItem("u10.fixture.config");
if (savedConfig) await fixture.core.handlers.get("config.set")!(JSON.parse(savedConfig), { window: "main", core: fixture.core });
const configSet = fixture.core.handlers.get("config.set")!;
fixture.core.handlers.set("config.set", (args, ctx) => { localStorage.setItem("u10.fixture.config", JSON.stringify(args)); return configSet(args, ctx); });
fixture.core.handlers.set("uistate.save", args => {
  fixture.core.uiState = args as UiStateJson; localStorage.setItem("u10.fixture.uiState", JSON.stringify(args)); return args;
});
window.__osMock = fixture.core;
registerMainRegions({ tabs: TopTabs, table: UpperTable });
const runtime = bootWindow({ kind: "main", transport: fixture.core.transport() });
await runtime.ready;
createRoot(document.getElementById("root")!).render(<StrictMode><App /></StrictMode>);
