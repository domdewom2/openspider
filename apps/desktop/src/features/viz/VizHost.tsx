// Einbindung im Hauptfenster: Aktionen des Menüs „Visualisierungen“ (UI-090, Einträge in ui/menu/contributions/main.ts);
// Fehler beim Öffnen als Banner.
import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { describeError, toCmdError } from "../../ipc/errors";
import { registerAction } from "../../ui/actions";
import { Banner } from "../../ui/primitives";
import { actionFor, MENU_KINDS } from "./menu";
import { openVisualisation } from "./open";

export function VizHost() {
  const t = useT();
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const offs = MENU_KINDS.map((kind) =>
      registerAction(actionFor(kind), () => {
        openVisualisation(kind).catch((e: unknown) => setError(describeError(t, toCmdError(e)).text));
      }),
    );
    return () => {
      for (const off of offs.reverse()) off();
    };
  }, [t]);

  if (!error) return null;
  return (
    <div className="os-viz-host-error">
      <Banner kind="error" onDismiss={() => setError(null)} dismissLabel={t("ui-dialog-close")}>
        {error}
      </Banner>
    </div>
  );
}
