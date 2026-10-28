import { useEffect, useLayoutEffect, useRef, useState, type DragEvent, type KeyboardEvent } from "react";
import { ChevronsRight } from "lucide-react";
import { Menu, type MenuEntry } from "./Menu";
import { IconButton } from "./Button";
import { cx } from "./cx";

export interface TabBarItem {
  id: string;
  label: string;
}

/**
 * Configuration of a tab bar (UI-004, UI-030, UI-060, UI-112): the same overflow menu on all three bars lists every tab with its
 * visibility, the tabs clipped by the width, „Tabs konfigurieren…“ and „Tabs zurücksetzen“ (A02-03, A02-06).
 */
export interface TabBarConfig {
  /** All tabs of the bar in their order, visible and hidden ones. */
  all: readonly TabBarItem[];
  hidden: readonly string[];
  /** The tab that cannot be hidden (top: Internal, detail: URL Details, sidebar: Overview). */
  mandatory?: string;
  mandatoryReason?: string;
  onToggle: (id: string, visible: boolean) => void;
  onConfigure: () => void;
  onReset: () => void;
  configureLabel: string;
  resetLabel: string;
  /** Disables visibility changes and reset while the stored layout is not loaded yet. */
  isDisabled?: boolean;
}

/**
 * Welche Tabs passen in die Breite? Der gewählte bleibt sichtbar; alles Übrige wandert in den Überlauf (»).
 * Reine Funktion, damit sie ohne Layout testbar ist.
 */
export function fitTabs(widths: readonly number[], available: number, overflowWidth: number, selectedIndex: number): { visible: number[]; hidden: number[] } {
  const total = widths.reduce((a, b) => a + b, 0);
  if (total <= available) return { visible: widths.map((_, i) => i), hidden: [] };
  const room = available - overflowWidth;
  const visible: number[] = [];
  let used = 0;
  const selectedWidth = widths[selectedIndex] ?? 0;
  for (let i = 0; i < widths.length; i++) {
    const w = widths[i] as number;
    const reserve = i < selectedIndex && !visible.includes(selectedIndex) ? selectedWidth : 0;
    if (used + w + reserve <= room) {
      visible.push(i);
      used += w;
    } else if (i === selectedIndex && !visible.includes(i)) {
      visible.push(i);
      used += w;
    }
  }
  if (!visible.includes(selectedIndex) && selectedIndex >= 0) visible.push(selectedIndex);
  visible.sort((a, b) => a - b);
  return { visible, hidden: widths.map((_, i) => i).filter((i) => !visible.includes(i)) };
}

/** Neue Reihenfolge nach Verschieben von `from` nach `to`. */
export function moveItem<T>(list: readonly T[], from: number, to: number): T[] {
  const copy = [...list];
  const [item] = copy.splice(from, 1);
  if (item === undefined || to < 0 || to >= list.length) return [...list];
  copy.splice(to, 0, item);
  return copy;
}

/** Entries of the overflow menu: visibility checkboxes for all tabs, the clipped tabs, then configure and reset. */
export function overflowMenuEntries(config: TabBarConfig, clipped: readonly TabBarItem[], onSelect: (id: string) => void): MenuEntry[] {
  return [
    ...config.all.map((tab) => ({
      id: `show-${tab.id}`, label: tab.label, checked: !config.hidden.includes(tab.id),
      isDisabled: config.isDisabled || tab.id === config.mandatory,
      disabledReason: tab.id === config.mandatory ? config.mandatoryReason : undefined,
      onCheckedChange: (checked: boolean) => config.onToggle(tab.id, checked),
    })),
    ...clipped.map((tab, i) => ({ id: `goto-${tab.id}`, label: tab.label, separatorBefore: i === 0, onAction: () => onSelect(tab.id) })),
    { id: "configure-tabs", label: config.configureLabel, separatorBefore: true, onAction: config.onConfigure },
    { id: "reset-tabs", label: config.resetLabel, isDisabled: config.isDisabled, onAction: config.onReset },
  ];
}

/**
 * Horizontale Tab-Leiste mit Überlauf-Dropdown und verschiebbarer Reihenfolge (Top-, Detail- und Seitenleisten-Tabs). Roving
 * Tabindex: ←/→ wechselt den Tab, Pos1/Ende springt; Alt+Umschalt+←/→ verschiebt den Tab (Tastatur-Alternative zum Ziehen).
 * Beschriftungen sind einzeilig; was nicht passt, endet mit „…“ und nennt den vollen Text im Tooltip (A02-01, A02-02, UI-007).
 * Mit `config` ist der Überlaufknopf immer da und sein Platz wird vor dem Verteilen der Tabs abgezogen (A01-08).
 */
export function TabBar({ tabs, selected, onSelect, label, overflowLabel, onReorder, measure, className, overflowEntries, config }: {
  /** Optional complete overflow/configuration menu (M2-U10); default lists clipped tabs. Ignored when `config` is set. */
  overflowEntries?: readonly MenuEntry[];
  tabs: readonly TabBarItem[];
  selected: string;
  onSelect: (id: string) => void;
  label: string;
  overflowLabel: string;
  onReorder?: (ids: string[]) => void;
  /** Breiten der Tabs und der Leiste; Standard: DOM-Messung (in Tests austauschbar). */
  measure?: (root: HTMLElement) => { widths: number[]; available: number; overflowWidth: number };
  className?: string;
  config?: TabBarConfig;
}) {
  const root = useRef<HTMLDivElement>(null);
  const [hidden, setHidden] = useState<number[]>([]);
  const [truncated, setTruncated] = useState<string[]>([]);
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const selectedIndex = Math.max(0, tabs.findIndex((t) => t.id === selected));
  const alwaysMenu = !!config || !!overflowEntries;

  // The latest measurement; one observer per bar calls it (a new observer per render reported "ResizeObserver loop" in WebKit).
  const measureNow = useRef<() => void>(() => undefined);
  measureNow.current = () => {
    const el = root.current;
    if (!el) return;
    let m;
    if (measure) m = measure(el);
    else if (el.clientWidth === 0) {
      // Not laid out (hidden pane, jsdom): nothing to distribute yet.
      m = { widths: [], available: 0, overflowWidth: 0 };
    } else {
      const trigger = el.querySelector<HTMLElement>(":scope > [data-tabbar-overflow]");
      const triggerWidth = trigger?.offsetWidth || 32;
      // An always present trigger takes its room before the tabs are distributed; otherwise it only appears on overflow.
      m = {
        widths: [...el.querySelectorAll<HTMLElement>("[data-tab-measure]")].map((t) => t.offsetWidth),
        available: el.clientWidth - (alwaysMenu ? triggerWidth : 0),
        overflowWidth: alwaysMenu ? 0 : triggerWidth,
      };
    }
    const next = fitTabs(m.widths, m.available, m.overflowWidth, selectedIndex).hidden;
    setHidden((old) => (old.length === next.length && old.every((v, i) => v === next[i]) ? old : next));
    const cut = [...el.querySelectorAll<HTMLElement>("[data-tab-id]")]
      .filter((t) => t.style.visibility !== "hidden")
      .filter((t) => { const text = t.querySelector<HTMLElement>(".os-tab-label") ?? t; return text.scrollWidth > text.clientWidth + 0.5; })
      .map((t) => t.dataset.tabId as string);
    setTruncated((old) => (old.length === cut.length && old.every((v, i) => v === cut[i]) ? old : cut));
  };
  useLayoutEffect(() => { measureNow.current(); }, [tabs, selectedIndex, measure, alwaysMenu, hidden]);
  useEffect(() => {
    const el = root.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    // Re-measure in the next frame: a distribution change inside the observer callback would raise "ResizeObserver loop" errors.
    let frame = 0;
    const ro = new ResizeObserver(() => { cancelAnimationFrame(frame); frame = requestAnimationFrame(() => measureNow.current()); });
    ro.observe(el);
    return () => { cancelAnimationFrame(frame); ro.disconnect(); };
  }, []);

  const move = (from: number, to: number) => onReorder?.(moveItem(tabs, from, to).map((t) => t.id));

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    // Keys from the overflow menu (a portal, bubbling through React) or its trigger do not move the tabs.
    const target = e.target as HTMLElement;
    if (!root.current?.contains(target) || target.closest?.("[data-tabbar-overflow]")) return;
    // Pfeiltasten gehen durch alle Tabs; ein versteckter Tab wird mit der Auswahl sichtbar
    const visible = tabs.map((_, i) => i);
    const pos = selectedIndex;
    if (e.altKey && e.shiftKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
      e.preventDefault();
      move(selectedIndex, selectedIndex + (e.key === "ArrowLeft" ? -1 : 1));
      return;
    }
    if (e.key === "Enter" || e.key === " ") {
      // Activation keys select the focused tab (WAI-ARIA tabs pattern; the sidebar's former RAC tabs did the same).
      const id = target.closest<HTMLElement>("[role=tab]")?.dataset.tabId;
      if (id) { e.preventDefault(); onSelect(id); }
      return;
    }
    let next: number | undefined;
    if (e.key === "ArrowRight") next = visible[Math.min(visible.length - 1, pos + 1)];
    else if (e.key === "ArrowLeft") next = visible[Math.max(0, pos - 1)];
    else if (e.key === "Home") next = visible[0];
    else if (e.key === "End") next = visible[visible.length - 1];
    if (next !== undefined) {
      e.preventDefault();
      const id = tabs[next]?.id;
      if (id) {
        onSelect(id);
        queueMicrotask(() => root.current?.querySelector<HTMLElement>(`[data-tab-id="${CSS.escape(id)}"]`)?.focus());
      }
    }
  };

  const clipped = hidden.map((i) => tabs[i]).filter((t): t is TabBarItem => !!t);
  const entries: readonly MenuEntry[] = config ? overflowMenuEntries(config, clipped, onSelect)
    : overflowEntries ?? clipped.map((t) => ({ id: t.id, label: t.label, onAction: () => onSelect(t.id) }));

  return (
    <div ref={root} className={cx("os-tabbar", className)} onKeyDown={onKeyDown}>
      <div role="tablist" aria-label={label} className="os-tablist os-tabbar-list">
        {tabs.map((t, i) => {
          const isHidden = hidden.includes(i) && t.id !== selected;
          const isSelected = t.id === selected;
          const isTruncated = truncated.includes(t.id);
          return (
            <div
              key={t.id}
              role="tab"
              data-tab-id={t.id}
              data-tab-measure=""
              data-truncated={isTruncated || undefined}
              title={isTruncated ? t.label : undefined}
              aria-selected={isSelected}
              tabIndex={isSelected ? 0 : -1}
              draggable={!!onReorder}
              className={cx("os-tab", isSelected && "os-tab-selected")}
              data-selected={isSelected || undefined}
              // Clipped tabs stay measurable (no `hidden`: a zero width would flip the distribution back and forth) but are
              // invisible, out of the tab order and out of the accessibility tree; the overflow menu lists them.
              style={isHidden && !isSelected ? { position: "absolute", visibility: "hidden", pointerEvents: "none" } : undefined}
              onClick={() => onSelect(t.id)}
              onDragStart={(e: DragEvent) => {
                setDragFrom(i);
                e.dataTransfer.effectAllowed = "move";
              }}
              onDragOver={(e: DragEvent) => {
                if (dragFrom !== null) e.preventDefault();
              }}
              onDrop={(e: DragEvent) => {
                e.preventDefault();
                if (dragFrom !== null && dragFrom !== i) move(dragFrom, i);
                setDragFrom(null);
              }}
              onDragEnd={() => setDragFrom(null)}
            >
              <span className="os-tab-label">{t.label}</span>
            </div>
          );
        })}
      </div>
      {(hidden.length > 0 || alwaysMenu) && (
        <span className="os-tabbar-overflow" data-tabbar-overflow="">
          <Menu label={overflowLabel} entries={entries} className="os-tabbar-menu" trigger={<IconButton label={overflowLabel} icon={<ChevronsRight size={14} />} />} />
        </span>
      )}
    </div>
  );
}
