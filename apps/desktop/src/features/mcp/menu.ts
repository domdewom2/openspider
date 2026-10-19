import type { MenuContribution } from "../../ui/menu/types";
import type { MenuEntry } from "../../ui/primitives";
import { runAction } from "../../ui/actions";
import type { TFn } from "../../i18n";

export function mcpMenu(running: boolean, autostart: boolean, busy: boolean, available: boolean): MenuContribution[] {
  return [{ menu: "mcp", group: "server", order: 10, items: [
    { id: "mcp.start", action: "mcp.start", labelKey: "mcp-start", enabled: () => available && !running && !busy },
    { id: "mcp.stop", action: "mcp.stop", labelKey: "mcp-stop", enabled: () => available && running && !busy },
    { id: "mcp.configure", action: "mcp.configure", labelKey: "mcp-configure" },
    { id: "mcp.autostart", action: "mcp.autostart", labelKey: "mcp-autostart", checked: () => autostart, enabled: () => available && !busy },
  ] }, { menu: "mcp", group: "tools", order: 20, items: [
    { id: "mcp.tools", action: "mcp.tools", labelKey: "mcp-tools" },
    { id: "mcp.markdown", action: "mcp.markdown", labelKey: "mcp-markdown", enabled: () => available && !busy },
    { id: "mcp.access", action: "mcp.access", labelKey: "mcp-access" },
  ] }, { menu: "mcp", group: "clients", order: 30, items: [
    { id: "mcp.claudeDesktop", action: "mcp.claudeDesktop", labelKey: "mcp-connect-desktop" },
    { id: "mcp.claudeCode", action: "mcp.claudeCode", labelKey: "mcp-connect-code" },
    { id: "mcp.codex", action: "mcp.codex", labelKey: "mcp-connect-codex" },
    { id: "mcp.log", action: "mcp.log", labelKey: "mcp-log" },
  ] }];
}

export function popupEntries(menu: MenuContribution[], t: TFn): MenuEntry[] {
  return menu.flatMap(group => group.items.map((item, i) => ({
    id: item.id, label: t(item.labelKey!), isDisabled: item.enabled?.({}) === false,
    ...(item.checked ? { checked: item.checked({}) } : {}),
    separatorBefore: i === 0,
    onAction: () => { runAction(item.action ?? item.id, { source: "menu" }); },
  })));
}
