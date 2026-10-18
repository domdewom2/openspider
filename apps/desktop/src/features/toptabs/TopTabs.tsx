import { useEffect } from "react";
import { useT } from "../../i18n";
import { eventBus } from "../../ipc/events";
import { useSession } from "../../state/session";
import { useTabs } from "../../state/tabs";
import { useAction } from "../../ui/actions";
import { registerMenuContributions } from "../../ui/menu/registry";
import { TabBar } from "../../ui/primitives/TabBar";
import { Button } from "../../ui/primitives/Button";

import "./toptabs.css";

export function TopTabs() {
  const t = useT(); const s = useTabs();
  const fail = (error: unknown) => useTabs.setState({ error });
  useEffect(() => { void useTabs.getState().load(); }, []);
  useEffect(() => eventBus.on("focusUrl", event => {
    if (useSession.getState().crawl?.crawlId !== event.crawlId) return;
    useTabs.getState().focus(event.isInternal ? "Internal" : "External", event.urlId);
  }), []);
  useEffect(() => registerMenuContributions("toptabs", [{ menu: "view", group: "tabs", order: 30, items: [
    { id: "view.resetTabs", labelKey: "u10-tabs-reset", action: "view.resetTabs" },
  ] }]), []);
  useAction("view.resetTabs", () => { void s.reset().catch(fail); });
  const move = (direction: number) => {
    const visible = s.order.filter(id => !s.hidden.includes(id));
    const index = visible.indexOf(s.active);
    s.select(visible[(index + direction + visible.length) % visible.length]);
  };
  useAction("toptab.next", () => move(1)); useAction("toptab.prev", () => move(-1));
  // One registration per shortcut, with a stable hook order.
  useAction("toptab.goto.1", () => s.select(s.order.filter(id => !s.hidden.includes(id))[0]));
  useAction("toptab.goto.2", () => s.select(s.order.filter(id => !s.hidden.includes(id))[1]));
  useAction("toptab.goto.3", () => s.select(s.order.filter(id => !s.hidden.includes(id))[2]));
  useAction("toptab.goto.4", () => s.select(s.order.filter(id => !s.hidden.includes(id))[3]));
  useAction("toptab.goto.5", () => s.select(s.order.filter(id => !s.hidden.includes(id))[4]));
  useAction("toptab.goto.6", () => s.select(s.order.filter(id => !s.hidden.includes(id))[5]));
  useAction("toptab.goto.7", () => s.select(s.order.filter(id => !s.hidden.includes(id))[6]));
  useAction("toptab.goto.8", () => s.select(s.order.filter(id => !s.hidden.includes(id))[7]));
  useAction("toptab.goto.9", () => s.select(s.order.filter(id => !s.hidden.includes(id))[8]));
  const label = (id: string) => t(`tab-${id.toLowerCase().replaceAll(" ", "-")}`, undefined, { fallback: id });
  return <div className="u10-top-tabs">
    <TabBar tabs={s.order.filter(id => !s.hidden.includes(id)).map(id => ({ id, label: label(id) }))}
      selected={s.active} onSelect={s.select} onReorder={ids => void s.reorder(ids).catch(fail)}
      label={t("u10-tabs")} overflowLabel={t("u10-tabs-overflow")} />
    <details className="u10-tabs-config">
      <summary aria-label={t("u10-tabs-configure")}>▾</summary>
      <div className="u10-tabs-popup" aria-label={t("u10-tabs-configure")}>
        {s.order.map((id, index) => <div className="u10-tabs-item" key={id}>
          <label><input type="checkbox" checked={!s.hidden.includes(id)} disabled={id === "Internal"}
            title={id === "Internal" ? t("u10-tabs-internal") : undefined}
            onChange={event => void s.show(id, event.target.checked).catch(fail)} />{label(id)}</label>
          <Button onPress={() => { void s.show(id, true).then(() => s.select(id)).catch(fail); }}>{t("u10-tabs-open")}</Button>
          <Button aria-label={t("u10-tabs-up", { tab: label(id) })} isDisabled={index === 0} onPress={() => {
            const order = [...s.order]; [order[index - 1], order[index]] = [order[index], order[index - 1]]; void s.reorder(order).catch(fail);
          }}>↑</Button>
          <Button aria-label={t("u10-tabs-down", { tab: label(id) })} isDisabled={index === s.order.length - 1} onPress={() => {
            const order = [...s.order]; [order[index + 1], order[index]] = [order[index], order[index + 1]]; void s.reorder(order).catch(fail);
          }}>↓</Button>
        </div>)}
        <Button onPress={() => void s.reset().catch(fail)}>{t("u10-tabs-reset")}</Button>
      </div>
    </details>
    {s.error != null && <span role="alert">{t("u10-tabs-error")}</span>}
  </div>;
}
