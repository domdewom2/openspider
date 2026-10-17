// Einstieg des Fensters `settings` (eigenes Fenster, nicht modal; Cmd+, bzw. Datei → Einstellungen).
import React from "react";
import ReactDOM from "react-dom/client";
import { SettingsWindow } from "../features/settings/window/SettingsWindow";
import { bootWindow } from "./runtime";
import "../ui/ui.css";

const runtime = bootWindow({ kind: "settings" });
void runtime.ready.then(() => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <SettingsWindow />
    </React.StrictMode>,
  );
});
