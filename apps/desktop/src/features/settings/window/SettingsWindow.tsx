import { useEffect, useState } from "react";
import { Providers } from "../../../app/Providers";
import { OPEN_SETTING_EVENT } from "../../../app/windows";
import { useWindowTitle } from "../../../app/windowTitle";
import { useT } from "../../../i18n";
import { isTauri } from "../../../ipc/transport/tauri";
import { SettingsFrame } from "../frame/SettingsFrame";

function readAnchor(): string | undefined {
  const h = typeof location !== "undefined" ? decodeURIComponent(location.hash.replace(/^#/, "")) : "";
  return h || undefined;
}

/** Eigenes Fenster `settings` (nicht modal): Anker aus der Adresse bzw. aus dem Fenster-Ereignis (UI-016). */
export function SettingsWindow() {
  const t = useT();
  const [anchor, setAnchor] = useState<string | undefined>(readAnchor);

  useWindowTitle(t("app-settings-title"));

  useEffect(() => {
    const onHash = () => setAnchor(readAnchor());
    window.addEventListener("hashchange", onHash);
    let unlisten: (() => void) | undefined;
    if (isTauri()) {
      void import("@tauri-apps/api/event").then(({ listen }) =>
        listen<string>(OPEN_SETTING_EVENT, (e) => setAnchor(e.payload)).then((u) => {
          unlisten = u;
        }),
      );
    }
    return () => {
      window.removeEventListener("hashchange", onHash);
      unlisten?.();
    };
  }, []);

  return (
    <Providers>
      <SettingsFrame {...(anchor ? { anchor } : {})} />
    </Providers>
  );
}
