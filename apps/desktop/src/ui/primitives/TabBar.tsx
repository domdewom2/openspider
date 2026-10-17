import { useLayoutEffect, useRef, useState, type DragEvent, type KeyboardEvent } from "react";
import { ChevronsRight } from "lucide-react";
import { Menu, type MenuEntry } from "./Menu";
import { IconButton } from "./Button";
import { cx } from "./cx";

export interface TabBarItem {
  id: string;
  label: string;
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

/**
 * Horizontale Tab-Leiste mit Überlauf-Dropdown und verschiebbarer Reihenfolge (Top-Tabs, Detail-Tabs). Roving Tabindex:
 * ←/→ wechselt den Tab, Pos1/Ende springt; Alt+Umschalt+←/→ verschiebt den Tab (Tastatur-Alternative zum Ziehen).
 */
export function TabBar({ tabs, selected, onSelect, label, overflowLabel, onReorder, measure, className }: {
  tabs: readonly TabBarItem[];
  selected: string;
  onSelect: (id: string) => void;
  label: string;
  overflowLabel: string;
  onReorder?: (ids: string[]) => void;
  /** Breiten der Tabs und der Leiste; Standard: DOM-Messung (in Tests austauschbar). */
  measure?: (root: HTMLElement) => { widths: number[]; available: number; overflowWidth: number };
  className?: string;
}) {
  const root = useRef<HTMLDivElement>(null);
  const [hidden, setHidden] = useState<number[]>([]);
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const selectedIndex = Math.max(0, tabs.findIndex((t) => t.id === selected));

  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    const run = () => {
      const m = measure
        ? measure(el)
        : {
            widths: [...el.querySelectorAll<HTMLElement>("[data-tab-measure]")].map((t) => t.offsetWidth),
            available: el.clientWidth,
            overflowWidth: 32,
          };
      setHidden(fitTabs(m.widths, m.available, m.overflowWidth, selectedIndex).hidden);
    };
    run();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(run);
    ro.observe(el);
    return () => ro.disconnect();
  }, [tabs, selectedIndex, measure]);

  const move = (from: number, to: number) => onReorder?.(moveItem(tabs, from, to).map((t) => t.id));

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    // Pfeiltasten gehen durch alle Tabs; ein versteckter Tab wird mit der Auswahl sichtbar
    const visible = tabs.map((_, i) => i);
    const pos = selectedIndex;
    if (e.altKey && e.shiftKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
      e.preventDefault();
      move(selectedIndex, selectedIndex + (e.key === "ArrowLeft" ? -1 : 1));
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

  const entries: MenuEntry[] = hidden.map((i) => ({ id: tabs[i]?.id ?? String(i), label: tabs[i]?.label ?? "", onAction: () => onSelect(tabs[i]?.id as string) }));

  return (
    <div ref={root} className={cx("os-tabbar", className)} onKeyDown={onKeyDown}>
      <div role="tablist" aria-label={label} className="os-tablist os-tabbar-list">
        {tabs.map((t, i) => {
          const isHidden = hidden.includes(i);
          const isSelected = t.id === selected;
          return (
            <div
              key={t.id}
              role="tab"
              data-tab-id={t.id}
              data-tab-measure=""
              aria-selected={isSelected}
              tabIndex={isSelected ? 0 : -1}
              hidden={isHidden && !isSelected}
              draggable={!!onReorder}
              className={cx("os-tab", isSelected && "os-tab-selected")}
              data-selected={isSelected || undefined}
              style={isHidden ? { position: "absolute", visibility: "hidden", pointerEvents: "none" } : undefined}
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
              {t.label}
            </div>
          );
        })}
      </div>
      {hidden.length > 0 && <Menu label={overflowLabel} entries={entries} trigger={<IconButton label={overflowLabel} icon={<ChevronsRight size={14} />} />} />}
    </div>
  );
}
