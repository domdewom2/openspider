import { useRef, useState, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";
import { Popover } from "react-aria-components";
import { MenuBody, type MenuEntry } from "./Menu";

/**
 * Kontextmenü (UI-110): Rechtsklick, Ctrl+Klick (macOS), Shift+F10 und Kontextmenütaste öffnen es an der Zeigerposition bzw.
 * am Element; Einträge ohne Voraussetzung sind ausgegraut und nennen den Grund.
 */
export function ContextMenu({ entries, label, children, className }: { entries: readonly MenuEntry[]; label: string; children: ReactNode; className?: string }) {
  const [at, setAt] = useState<{ x: number; y: number } | null>(null);
  const anchor = useRef<HTMLDivElement>(null);
  const host = useRef<HTMLDivElement>(null);

  const open = (x: number, y: number) => {
    const rect = host.current?.getBoundingClientRect();
    setAt({ x: x - (rect?.left ?? 0), y: y - (rect?.top ?? 0) });
  };
  const onContextMenu = (e: MouseEvent) => {
    e.preventDefault();
    open(e.clientX, e.clientY);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey)) {
      e.preventDefault();
      const r = (e.target as HTMLElement).getBoundingClientRect();
      open(r.left + 8, r.bottom);
    }
  };

  return (
    <div ref={host} className={className} onContextMenu={onContextMenu} onKeyDown={onKeyDown} style={{ position: "relative" }}>
      {children}
      <div ref={anchor} aria-hidden style={{ position: "absolute", left: at?.x ?? 0, top: at?.y ?? 0, width: 0, height: 0 }} />
      <Popover triggerRef={anchor} isOpen={at !== null} onOpenChange={(o) => !o && setAt(null)} placement="bottom start" className="os-popover" aria-label={label}>
        <MenuBody entries={entries.map((e) => ({ ...e, onAction: () => { e.onAction?.(); setAt(null); } }))} label={label} />
      </Popover>
    </div>
  );
}
