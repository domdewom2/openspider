import { useState } from "react";
import { useT } from "../../i18n";
import { isHexColor } from "../tokens/color";
import { accentInfo, ACCENT_PALETTE, DEFAULT_ACCENT_DISPLAY, displayAccent, type ThemeName } from "../tokens/tokens";
import { cx } from "./cx";
import { TextField } from "./fields";

/**
 * Akzentfarbe wählen: acht vorgeprüfte Farben plus freie Eingabe (`#rrggbb`). Unter 3:1 gegen den Hintergrund des
 * aktiven Themes erscheint eine Warnung mit dem Kontrastwert; Speichern bleibt möglich (UI-120).
 */
export function ColorPicker({ value: storedValue, onChange, theme, label, anchor }: { value: string; onChange: (hex: string) => void; theme: ThemeName; label: string; anchor?: string }) {
  const value = displayAccent(storedValue);
  const t = useT();
  const [draft, setDraft] = useState(value);
  const [synced, setSynced] = useState(value);
  if (synced !== value) {
    setSynced(value);
    setDraft(value);
  }
  const valid = isHexColor(draft);
  const info = accentInfo(isHexColor(value) ? value : DEFAULT_ACCENT_DISPLAY, theme);
  return (
    <div className="os-colorpicker" data-setting-anchor={anchor} role="group" aria-label={label}>
      <div className="os-swatches">
        {ACCENT_PALETTE.map((c) => {
          const selected = c.hex.toLowerCase() === value.toLowerCase();
          return (
            <button
              key={c.id}
              type="button"
              className={cx("os-swatch", selected && "os-swatch-selected")}
              style={{ background: c.hex }}
              aria-label={t(`color-${c.id}`)}
              aria-pressed={selected}
              onClick={() => onChange(c.hex.toLowerCase())}
            />
          );
        })}
      </div>
      <TextField
        label={t("color-custom")}
        value={draft}
        placeholder="#rrggbb"
        error={valid || draft === "" ? null : t("color-invalid")}
        onChange={(v) => {
          setDraft(v);
          if (isHexColor(v)) onChange(v.toLowerCase());
        }}
      />
      {info.warn && (
        <p className="os-error" role="alert" data-testid="accent-warning">
          {t("color-low-contrast", { ratio: info.minContrast.toFixed(1) })}
        </p>
      )}
    </div>
  );
}
