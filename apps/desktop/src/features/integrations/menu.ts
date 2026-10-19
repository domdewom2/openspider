// Menübeiträge: „Konfiguration → API-Zugriff“ mit Untermenü je Quelle und „Modus → APIs“ (INTEG-001, INTEG-012).
import type { MenuContribution } from "../../ui/menu/types";
import { DIALOG_KINDS } from "./sources";

export function integrationsMenu(apiMode: boolean): MenuContribution[] {
  return [
    {
      menu: "config",
      group: "integrations",
      order: 60,
      items: [
        {
          id: "config.apiAccess",
          labelKey: "integ-menu-api-access",
          items: [
            { id: "config.apiAccess.all", labelKey: "integ-menu-overview", action: "integrations.open" },
            { id: "config.apiAccess.sep", separator: true },
            ...DIALOG_KINDS.map((k) => ({ id: `config.apiAccess.${k}`, labelKey: `api-${k}`, action: `integrations.open.${k}` })),
          ],
        },
      ],
    },
    {
      menu: "mode",
      group: "variants",
      order: 40,
      items: [{ id: "mode.apis", labelKey: "integ-menu-mode-apis", action: "mode.apis", checked: () => apiMode }],
    },
  ];
}
