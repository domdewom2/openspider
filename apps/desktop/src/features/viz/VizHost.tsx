// Einbindung im Hauptfenster: Menübeiträge und Aktionen „Visualisierungen“ (UI-090); Fehler beim Öffnen als Banner.
import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { describeError, toCmdError } from "../../ipc/errors";
import { registerAction } from "../../ui/actions";
import { registerMenuContributions } from "../../ui/menu/registry";
import { Banner } from "../../ui/primitives";
import { actionFor, VIZ_MENU } from "./menu";
import { GRAPH_KINDS, openVisualisation } from "./open";

export function VizHost() {
  const t = useT();
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const offs = GRAPH_KINDS.map((kind) =>
      registerAction(actionFor(kind), () => {
        openVisualisation(kind).catch((e: unknown) => setError(describeError(t, toCmdError(e)).text));
      }),
    );
    offs.push(registerMenuContributions("features/viz", VIZ_MENU));
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
