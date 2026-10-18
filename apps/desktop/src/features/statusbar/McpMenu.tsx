import { useEffect, useState } from "react";
import { openSettingsWindow } from "../../app/windows";
import { useLanguageStore, useT } from "../../i18n";
import { useSession } from "../../state/session";
import { runAction } from "../../ui/actions";
import { buildMenuModel } from "../../ui/menu/build";
import { allMenuContributions, onMenuContributionsChanged } from "../../ui/menu/registry";
import type { ResolvedItem } from "../../ui/menu/types";
import { currentPlatform } from "../../ui/platform";
import { Button } from "../../ui/primitives/Button";
import { Menu, type MenuEntry } from "../../ui/primitives/Menu";
import type { ReactNode } from "react";

/** The status field opens the same registered MCP actions as the main menu. */
export function McpMenu({ children }: { children: ReactNode }) {
  const t = useT();
  const lang = useLanguageStore((s) => s.lang);
  const [, changed] = useState(0);
  useEffect(() => onMenuContributionsChanged(() => changed((n) => n + 1)), []);
  const crawl = useSession.getState().crawl;
  const model = buildMenuModel(allMenuContributions(), { platform: currentPlatform(), lang, t, ctx: { hasCrawl: !!crawl, crawlRunning: crawl?.phase === "running" }, native: false });
  function entries(items: ResolvedItem[], parent = ""): MenuEntry[] {
    return items.flatMap((item) => item.kind === "separator" ? [] : item.items ? entries(item.items, `${parent}${item.label}: `) : [{
      id: item.id, label: parent + item.label, isDisabled: !item.enabled, disabledReason: item.disabledReason,
      shortcut: item.shortcutText, onAction: () => { if (item.action) runAction(item.action); },
    }]);
  }
  const actions = entries(model.menus.find((menu) => menu.id === "mcp")?.items ?? []);
  if (actions.length === 0) actions.push({ id: "mcp.settings", label: t("status-mcp-settings"), onAction: () => void openSettingsWindow("mcp") });
  return <Menu label="MCP" entries={actions} trigger={<Button variant="tool" className="os-mcp-status" onPress={() => { runAction("mcp.menu"); }}>{children}</Button>} />;
}
