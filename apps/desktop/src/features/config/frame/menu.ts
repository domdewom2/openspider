// Menübeiträge „Konfiguration → Spider… / Profile… / Abweichungen anzeigen…“ (OUT-220, OUT-224).
import type { MenuContribution } from "../../../ui/menu/types";

export function configMenu(): MenuContribution[] {
  return [
    {
      menu: "config",
      group: "dialog",
      order: 10,
      items: [
        { id: "config.spider", labelKey: "config-menu-spider", action: "config.spider" },
        { id: "config.profiles", labelKey: "config-menu-profiles", action: "config.profiles" },
        { id: "config.diff", labelKey: "config-menu-diff", action: "config.diff" },
      ],
    },
  ];
}
