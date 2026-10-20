// Einmal je Hauptfenster: registriert die Aktionen (Kürzel Mod+K, Mod+Umschalt+C), die Menübeiträge und hält den Dialog.
import { useEffect } from "react";
import { registerAction } from "../../../ui/actions";
import { registerMenuContributions } from "../../../ui/menu/registry";
import { ConfigDialog } from "./ConfigDialog";
import { configMenu } from "./menu";
import { openConfig, useConfigDialog } from "./store";

export function ConfigHost() {
  const open = useConfigDialog((s) => s.open);
  useEffect(() => {
    const offs = [
      registerAction("config.spider", () => openConfig()),
      registerAction("config.profiles", () => openConfig({ anchor: "profiles" })),
      registerAction("config.diff", () => openConfig({ diff: true })),
    ];
    const offMenu = registerMenuContributions("features/config", configMenu());
    return () => {
      for (const off of offs.reverse()) off();
      offMenu();
    };
  }, []);
  return open ? <ConfigDialog /> : null;
}
