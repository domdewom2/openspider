import type { ReactNode } from "react";
import { Button as RacButton, ComboBox as RacComboBox, Input, Label, ListBox, ListBoxItem, Popover } from "react-aria-components";
import { ChevronDown } from "lucide-react";
import { cx } from "./cx";

export interface ComboOption {
  id: string;
  label: string;
  /** Zähler hinter dem Namen: „Name (Anzahl)“ (Filter-Dropdown, UI-006); `null` = „–“. */
  count?: number | null;
  /** Kennzeichen vor dem Namen (Typsymbol, Hinweissymbol). */
  badge?: ReactNode;
  /** Zusatztext für Screenreader, z. B. „Crawl-Analyse erforderlich“. */
  hint?: string;
  isDisabled?: boolean;
}

/** Auswahl mit Texteingabe (Filter-Dropdown mit Zähler und Badges); RAC liefert `combobox`-Semantik und Tastatur. */
export function ComboBox({ label, options, selected, onSelect, formatCount = (n) => String(n), className }: {
  label: string;
  options: readonly ComboOption[];
  selected: string | null;
  onSelect: (id: string) => void;
  formatCount?: (n: number) => string;
  className?: string;
}) {
  const text = (o: ComboOption) => (o.count === undefined ? o.label : `${o.label} (${o.count === null ? "–" : formatCount(o.count)})`);
  return (
    <RacComboBox
      className={cx("os-field", "os-combobox", className)}
      selectedKey={selected}
      onSelectionChange={(k) => {
        if (k !== null) onSelect(String(k));
      }}
      defaultFilter={(textValue, input) => textValue.toLowerCase().includes(input.toLowerCase())}
    >
      <Label className="os-label">{label}</Label>
      <div className="os-combobox-group">
        <Input className="os-input" />
        <RacButton className="os-combobox-button" aria-label={label}>
          <ChevronDown size={14} aria-hidden />
        </RacButton>
      </div>
      <Popover className="os-popover">
        <ListBox className="os-listbox">
          {options.map((o) => (
            <ListBoxItem key={o.id} id={o.id} textValue={text(o)} isDisabled={o.isDisabled} className="os-listbox-item" aria-description={o.hint}>
              <span className="os-combobox-item">
                {o.badge}
                <span>{o.label}</span>
              </span>
              {o.count !== undefined && <span className="os-keyhint">{o.count === null ? "–" : formatCount(o.count)}</span>}
            </ListBoxItem>
          ))}
        </ListBox>
      </Popover>
    </RacComboBox>
  );
}
