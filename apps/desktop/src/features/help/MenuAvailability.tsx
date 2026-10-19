import { useSyncExternalStore } from "react";
import { useT, useLanguageStore } from "../../i18n";
import { useSession } from "../../state/session";
import { currentPlatform } from "../../ui/platform";
import { buildMenuModel, flattenItems } from "../../ui/menu/build";
import { allMenuContributions, onMenuContributionsChanged } from "../../ui/menu/registry";

let revision = 0;
const subscribe = (changed: () => void) => onMenuContributionsChanged(() => { revision++; changed(); });
const snapshot = () => revision;

/** Native menu APIs expose no hover event; the status line provides the reasons by action name. */
export function MenuAvailability() {
  useSyncExternalStore(subscribe, snapshot);
  const t = useT();
  const lang = useLanguageStore(s => s.lang);
  const { crawl, analysis } = useSession();
  const model = buildMenuModel(allMenuContributions(), {
    platform: currentPlatform(), lang, t, native: false,
    ctx: { hasCrawl: !!crawl, crawlRunning: crawl?.phase === "running", hasData: (crawl?.crawled ?? 0) > 0, analysisRunning: analysis?.running === true },
  });
  const entries = [...new Map(flattenItems(model.menus.flatMap(menu => menu.items)).filter(item => !item.enabled && item.disabledReason).map(item => [item.id, item])).values()];
  if (!entries.length) return null;
  return <details className="os-menu-availability">
    <summary>{t("u04-unavailable-actions", { count: entries.length })}</summary>
    <ul>{entries.map(item => <li key={item.id}>{item.label}: {item.disabledReason}</li>)}</ul>
  </details>;
}
