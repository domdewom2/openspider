import type { ReactNode } from "react";
import { Menu as RacMenu, MenuItem, MenuSection, MenuTrigger, Popover, Separator } from "react-aria-components";
import { cx } from "./cx";

export interface MenuEntry {
  /** Optional checkbox state; omitted entries retain ordinary action semantics. */
  checked?: boolean;
  onCheckedChange?: (checked: boolean) => void;
  id: string;
  label: string;
  onAction?: () => void;
  isDisabled?: boolean;
  /** Grund für „deaktiviert“ (UI-002/UI-110: ausgegraut mit Begründung). */
  disabledReason?: string;
  /** Anzeige des Kürzels (KeyHint-Text). */
  shortcut?: string;
  separatorBefore?: boolean;
}

/** Menü mit Auslöser (Schaltfläche) – ausgegraute Einträge nennen ihren Grund über `title`. */
export function Menu({ trigger, entries, label, className }: { trigger: ReactNode; entries: readonly MenuEntry[]; label: string; className?: string }) {
  return (
    <MenuTrigger>
      {trigger}
      <Popover className="os-popover">
        <MenuBody entries={entries} label={label} className={className} />
      </Popover>
    </MenuTrigger>
  );
}

export function MenuBody({ entries: all, label, className }: { entries: readonly MenuEntry[]; label: string; className?: string }) {
  // Collections need unique keys; a duplicate id (e.g. one action contributed twice to a merged menu) would stop the whole menu
  // from rendering, so only its first occurrence is kept (M2-U34, Fremdänderung).
  const entries = all.filter((e, i) => all.findIndex(x => x.id === e.id) === i);
  return (
    <RacMenu className={cx("os-menu", className)} aria-label={label}>
      {entries.some(e => e.checked !== undefined) && <MenuSection selectionMode="multiple"
        selectedKeys={entries.filter(e => e.checked).map(e => e.id)} shouldCloseOnSelect={false}
        onSelectionChange={keys => {
          for (const entry of entries.filter(e => e.checked !== undefined && !e.isDisabled)) {
            const checked = keys === "all" || keys.has(entry.id);
            if (checked !== entry.checked) {
              if (entry.onCheckedChange) entry.onCheckedChange(checked);
              else entry.onAction?.();
            }
          }
        }}>
        {entries.filter(e => e.checked !== undefined).map(e =>
          <MenuItem key={e.id} id={e.id} className="os-menu-item" isDisabled={e.isDisabled} textValue={e.label}
            {...(e.disabledReason ? { "aria-description": e.disabledReason, title: e.disabledReason } : {})}>
            <span aria-hidden>{e.checked ? "☑" : "☐"} </span><span>{e.label}</span>
          </MenuItem>)}
      </MenuSection>}
      {entries.filter(e => e.checked === undefined).flatMap((e) => [
        ...(e.separatorBefore ? [<Separator key={`${e.id}-sep`} className="os-menu-sep" />] : []),
        <MenuItem key={e.id} id={e.id} className="os-menu-item" isDisabled={e.isDisabled} onAction={e.onAction} textValue={e.label} {...(e.disabledReason ? { "aria-description": e.disabledReason, title: e.disabledReason } : {})}>
          <span>{e.label}</span>
          {e.shortcut && <kbd className="os-keyhint">{e.shortcut}</kbd>}
        </MenuItem>,
      ])}
    </RacMenu>
  );
}
