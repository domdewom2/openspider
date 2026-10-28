import { useState } from "react";
import { useT } from "../../i18n";
import { columnLabel } from "../../i18n/canon";
import { type ColumnState } from "../../grid/DataGrid";
import { Dialog, DialogFooter } from "../../ui/primitives/Dialog";
import { Button } from "../../ui/primitives/Button";

export function ColumnsMenu({ columns, onChange, onReset }: {
  columns: ColumnState[]; onChange: (columns: ColumnState[]) => void; onReset: () => void;
}) {
  const t = useT(); const [open, setOpen] = useState(false); const [search, setSearch] = useState("");
  const [draft, setDraft] = useState(columns); const [initial, setInitial] = useState(columns); const [selected, setSelected] = useState<string[]>([]);
  const [drag, setDrag] = useState<string>(); const [target, setTarget] = useState<string>();
  const fixed = (key: string) => key === "Row" || key === "Address";
  const move = (direction: number) => {
    const copy = [...draft];
    const indices = copy.map((c, i) => selected.includes(c.key) && !fixed(c.key) ? i : -1).filter(i => i >= 0);
    if (direction > 0) indices.reverse();
    for (const from of indices) {
      const to = direction === -Infinity ? 2 : direction === Infinity ? copy.length - 1 : from + direction;
      if (to < 2 || to >= copy.length || selected.includes(copy[to].key)) continue;
      const [c] = copy.splice(from, 1); copy.splice(to, 0, c);
    }
    setDraft(copy);
  };
  return <>
    <Button aria-label={t("u10-columns")} onPress={() => { setDraft(columns); setInitial(columns); setSelected([]); setOpen(true); }}>+</Button>
    <Dialog title={t("u10-columns")} isOpen={open} onOpenChange={value => { if (!value) onChange(initial); setOpen(value); }}
      footer={<DialogFooter okLabel={t("u10-apply")} cancelLabel={t("u10-cancel")}
        onOk={() => { onChange(draft); setOpen(false); }} onCancel={() => { onChange(initial); setOpen(false); }}
        extra={<Button onPress={() => { onReset(); setOpen(false); }}>{t("u10-columns-reset")}</Button>} />}>
      <label>{t("u10-columns-search")}<input value={search} onChange={e => setSearch(e.target.value)} /></label>
      <Button onPress={() => { const next = draft.map(c => ({ ...c, visible: true })); setDraft(next); onChange(next); }}>{t("u10-columns-all")}</Button>
      <Button onPress={() => { const next = draft.map(c => ({ ...c, visible: fixed(c.key) })); setDraft(next); onChange(next); }}>{t("u10-columns-none")}</Button>
      <div className="u10-column-list">
        {draft.filter(c => `${c.key} ${columnLabel(t, c.key)}`.toLowerCase().includes(search.toLowerCase())).map(c => <div key={c.key}
          className="u10-column-item" data-drop-target={target === c.key || undefined} draggable={!fixed(c.key)}
          onDragStart={() => setDrag(c.key)} onDragEnd={() => { setDrag(undefined); setTarget(undefined); }}
          onDragOver={e => { if (!fixed(c.key)) { e.preventDefault(); setTarget(c.key); } }}
          onDrop={e => {
            e.preventDefault(); if (!drag || fixed(c.key)) return;
            const next = draft.filter(d => d.key !== drag); next.splice(next.findIndex(d => d.key === c.key), 0, draft.find(d => d.key === drag)!);
            setDraft(next); onChange(next); setDrag(undefined); setTarget(undefined);
          }}>
          <label><input type="checkbox" checked={c.visible} disabled={fixed(c.key)} onChange={e => {
            const next = draft.map(d => d.key === c.key ? { ...d, visible: e.target.checked } : d); setDraft(next); onChange(next);
          }} />{columnLabel(t, c.key)}</label>
          {!fixed(c.key) && <input type="checkbox" aria-label={t("u10-columns-select", { column: columnLabel(t, c.key) })} checked={selected.includes(c.key)}
            onChange={e => setSelected(e.target.checked ? [...selected, c.key] : selected.filter(key => key !== c.key))} />}
        </div>)}
      </div>
      <div className="u10-column-actions">
        <Button onPress={() => move(-Infinity)}>{t("u10-first")}</Button><Button onPress={() => move(-1)}>{t("u10-up")}</Button>
        <Button onPress={() => move(1)}>{t("u10-down")}</Button><Button onPress={() => move(Infinity)}>{t("u10-last")}</Button>
      </div>
    </Dialog>
  </>;
}
