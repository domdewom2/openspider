import { useRef, useState, type KeyboardEvent, type PointerEvent, type ReactNode } from "react";
import { cx } from "./cx";

/** Begrenzt die Größe des ersten Bereichs so, dass beide Bereiche ihr Mindestmaß behalten. */
export function clampSplit(total: number, size: number, min1: number, min2: number, grip = 4): number {
  const max = Math.max(min1, total - grip - min2);
  return Math.min(Math.max(size, min1), max);
}

export interface SplitPaneProps {
  orientation: "horizontal" | "vertical";
  /** Größe des ersten Bereichs in px. */
  size: number;
  onSizeChange: (size: number) => void;
  min1?: number;
  min2?: number;
  /** Doppelklick auf den Trenner setzt auf diese Größe zurück. */
  defaultSize?: number;
  collapsed?: boolean;
  label: string;
  first: ReactNode;
  second: ReactNode;
  className?: string;
}

const STEP = 8;

/** Teilung mit Trenner (4 px sichtbar, 8 px Greiffläche); Pfeiltasten, Pos1/Ende und Doppelklick-Reset. */
export function SplitPane({ orientation, size, onSizeChange, min1 = 120, min2 = 120, defaultSize, collapsed, label, first, second, className }: SplitPaneProps) {
  const root = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState(false);
  const horizontal = orientation === "horizontal";
  const total = () => (horizontal ? (root.current?.clientWidth ?? 0) : (root.current?.clientHeight ?? 0));
  const apply = (next: number) => onSizeChange(clampSplit(total() || Infinity, next, min1, min2));

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    setDragging(true);
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!dragging || !root.current) return;
    const rect = root.current.getBoundingClientRect();
    apply(horizontal ? e.clientX - rect.left : e.clientY - rect.top);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const dec = horizontal ? "ArrowLeft" : "ArrowUp";
    const inc = horizontal ? "ArrowRight" : "ArrowDown";
    if (e.key === dec) apply(size - STEP);
    else if (e.key === inc) apply(size + STEP);
    else if (e.key === "Home") apply(min1);
    else if (e.key === "End") apply(total());
    else return;
    e.preventDefault();
  };

  return (
    <div ref={root} className={cx("os-split", `os-split-${orientation}`, className)} data-dragging={dragging || undefined}>
      <div className="os-split-pane" style={collapsed ? { display: "none" } : horizontal ? { width: size } : { height: size }}>
        {first}
      </div>
      <div
        role="separator"
        tabIndex={0}
        aria-label={label}
        aria-orientation={horizontal ? "vertical" : "horizontal"}
        aria-valuenow={Math.round(size)}
        aria-valuemin={min1}
        className="os-splitter"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={() => setDragging(false)}
        onKeyDown={onKeyDown}
        onDoubleClick={() => defaultSize !== undefined && apply(defaultSize)}
      />
      <div className="os-split-pane os-split-fill">{second}</div>
    </div>
  );
}
