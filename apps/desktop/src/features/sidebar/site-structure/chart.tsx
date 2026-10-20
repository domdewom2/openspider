import { useFormat } from "../../../i18n";
import "./chart.css";

export interface BarPart { key: string; label: string; value: number; color: string }
export interface BarRow { key: string; label: string; total: number; parts: readonly BarPart[] }

/**
 * Horizontal (stacked) bar chart shared by Site Structure, Segments and Response Times. Every segment is a button:
 * tooltip and accessible name carry the absolute number; a click reports `(row, part)` (UI-066, UI-069).
 */
export function Bars({ label, rows, onSelect }: { label: string; rows: readonly BarRow[]; onSelect?: (row: string, part: string) => void }) {
  const f = useFormat();
  const max = Math.max(1, ...rows.map(r => r.total));
  return <ul className="os-sidebar-bars" aria-label={label}>
    {rows.map(row => <li key={row.key}>
      <span className="os-sidebar-bar-label">{row.label}</span>
      <span className="os-sidebar-bar" style={{ width: `${(row.total / max) * 100}%` }} role="group" aria-label={`${row.label}: ${f.integer(row.total)}`}>
        {row.parts.filter(p => p.value > 0).map(p => {
          const text = `${row.label} – ${p.label}: ${f.integer(p.value)}`;
          return <button type="button" key={p.key} className="os-sidebar-bar-part" data-class={p.key} title={text} aria-label={text} style={{ flexGrow: p.value, background: p.color }} onClick={() => onSelect?.(row.key, p.key)} />;
        })}
      </span>
    </li>)}
  </ul>;
}
