// Fenster `viz-*` (Visualisierungen, nicht modal, M2-U24).
import React from "react";
import ReactDOM from "react-dom/client";
import { readWindowParams, VizWindow } from "../features/viz";
import { bootWindow } from "./runtime";
import "../ui/ui.css";

const params = readWindowParams(location.search);
const runtime = bootWindow({ kind: "viz", label: params.label });
void runtime.ready.then(() => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <VizWindow params={params} exportDir={runtime.transportName === "tauri" ? null : new URLSearchParams(location.search).get("exportDir")} />
    </React.StrictMode>,
  );
});
