// Fenster `viz-*` (Visualisierungen, nicht modal): Gerüst – der Inhalt kommt mit M2-U24.
import React from "react";
import ReactDOM from "react-dom/client";
import { useT } from "../i18n";
import { EmptyState } from "../ui/primitives/EmptyState";
import { bootWindow } from "./runtime";
import "../ui/ui.css";

function VizPlaceholder() {
  const t = useT();
  return (
    <main aria-label={t("app-viz-title")} style={{ height: "100%" }}>
      <EmptyState state={{ code: "NO_DATA_YET" }} />
    </main>
  );
}

const runtime = bootWindow({ kind: "viz" });
void runtime.ready.then(() => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <VizPlaceholder />
    </React.StrictMode>,
  );
});
