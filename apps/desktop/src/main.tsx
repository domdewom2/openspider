import React from "react";
import ReactDOM from "react-dom/client";
import { App, registerMainRegions } from "./App";
import { DetailFrame } from "./features/detail/frame/DetailFrame";
import { bootWindow } from "./app/runtime";
import "./ui/ui.css";
import "./app/app.css";

const params = new URLSearchParams(location.search);
const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);

if (import.meta.env.DEV && params.has("harness")) {
  // Harness für visuelle Tests: nur im Dev-Build, deterministische Daten über den Mock-Transport.
  void import("./harness/entry").then((m) => m.mountHarness(root));
} else {
  registerMainRegions({ detail: DetailFrame });
  const runtime = bootWindow({ kind: "main" });
  void runtime.ready.then(() => {
    root.render(
      <React.StrictMode>
        <App />
      </React.StrictMode>,
    );
  });
}
