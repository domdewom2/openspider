// Einbindung ins Hauptfenster: Dialog-Host, Aktionen, Menübeiträge, Zahnrad je Quelle (UI-070) und der Platz in der
// Kopfleiste (API-Fortschritt, APIs-Modus als Variante).
import { useEffect } from "react";
import { useT } from "../../i18n";
import type { ApiKind } from "../../ipc/gen/ApiKind";
import { registerAction } from "../../ui/actions";
import { Settings } from "../../ui/icons";
import { registerMenuContributions } from "../../ui/menu/registry";
import { IconButton } from "../../ui/primitives";
import { ApiAccessDialog } from "./ApiAccessDialog";
import { ApiModeBar } from "./ApiModeBar";
import { ApiProgress } from "./ApiProgress";
import { integrationsMenu } from "./menu";
import { DIALOG_KINDS } from "./sources";
import { openApiAccess, setApiMode, useIntegrations } from "./store";

/** Zahnrad einer Quellenzeile (Seitenleiste „API“, UI-070): öffnet den Verbindungsdialog genau dieser Quelle. */
export function ApiSourceGear({ kind }: { kind: ApiKind }) {
  const t = useT();
  return <IconButton label={t("integ-gear", { source: t(`api-${kind}`) })} icon={<Settings size={14} />} onPress={() => openApiAccess(kind)} />;
}

/** Einmal je Hauptfenster: registriert Aktionen und Menübeiträge und hält den Dialog. */
export function IntegrationsHost() {
  const apiMode = useIntegrations((s) => s.apiMode);
  useEffect(() => {
    const offs = [
      registerAction("integrations.open", () => openApiAccess()),
      registerAction("mode.apis", () => setApiMode(!useIntegrations.getState().apiMode)),
      ...DIALOG_KINDS.map((k) => registerAction(`integrations.open.${k}`, () => openApiAccess(k))),
    ];
    return () => {
      for (const off of offs.reverse()) off();
    };
  }, []);
  useEffect(() => registerMenuContributions("features/integrations", integrationsMenu(apiMode)), [apiMode]);
  return <ApiAccessDialog />;
}

/** Platz in der Kopfleiste (U03 bindet ihn ein): API-Fortschritt und, im APIs-Modus, die Listeneingabe. */
export function IntegrationsHeaderSlot() {
  const apiMode = useIntegrations((s) => s.apiMode);
  return (
    <div className="os-integ-header">
      {apiMode && <ApiModeBar />}
      <ApiProgress />
    </div>
  );
}
