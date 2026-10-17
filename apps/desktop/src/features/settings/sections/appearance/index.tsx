import { useState } from "react";
import { useT } from "../../../../i18n";
import { LIVE_INTERVAL_RANGE, usePrefs } from "../../../../state/prefs";
import { Button, ColorPicker, RadioGroup, Select, Switch, TextField } from "../../../../ui/primitives";
import { resolveTheme, systemPrefersDark } from "../../../../ui/tokens/theme";
import { SIZES, ZOOM_STEPS } from "../../../../ui/tokens/tokens";
import type { SettingsSection } from "../../frame/types";
import { sectionTitleKey } from "../../frame/types";

function LiveInterval() {
  const t = useT();
  const ms = usePrefs((s) => s.liveIntervalMs);
  const patch = usePrefs((s) => s.patch);
  const [draft, setDraft] = useState(String(ms));
  const [seen, setSeen] = useState(ms);
  if (seen !== ms) {
    setSeen(ms);
    setDraft(String(ms));
  }
  const n = Number(draft);
  const valid = Number.isInteger(n) && n >= LIVE_INTERVAL_RANGE.min && n <= LIVE_INTERVAL_RANGE.max;
  return (
    <TextField
      label={t("settings-appearance-interval")}
      description={t("settings-appearance-interval-hint", { min: LIVE_INTERVAL_RANGE.min, max: LIVE_INTERVAL_RANGE.max })}
      value={draft}
      type="number"
      inputMode="numeric"
      anchor="appearance.liveInterval"
      error={valid ? null : t("settings-appearance-interval-invalid", { min: LIVE_INTERVAL_RANGE.min, max: LIVE_INTERVAL_RANGE.max })}
      onChange={(v) => {
        setDraft(v);
        const x = Number(v);
        if (Number.isInteger(x) && x >= LIVE_INTERVAL_RANGE.min && x <= LIVE_INTERVAL_RANGE.max) void patch({ appearance: { liveIntervalMs: x } });
      }}
    />
  );
}

function Appearance() {
  const t = useT();
  const a = usePrefs((s) => s.appearance);
  const patch = usePrefs((s) => s.patch);
  const reset = usePrefs((s) => s.resetAppearance);
  const theme = resolveTheme(a.theme, systemPrefersDark());
  return (
    <div className="os-settings-group">
      <RadioGroup
        label={t("settings-appearance-theme")}
        value={a.theme}
        anchor="appearance.theme"
        orientation="horizontal"
        options={[
          { value: "light", label: t("settings-appearance-theme-light") },
          { value: "dark", label: t("settings-appearance-theme-dark") },
          { value: "system", label: t("settings-appearance-theme-system") },
        ]}
        onChange={(v) => void patch({ appearance: { theme: v } })}
      />
      <ColorPicker label={t("settings-appearance-accent")} anchor="appearance.accent" value={a.accent} theme={theme} onChange={(hex) => void patch({ appearance: { accent: hex } })} />
      <Select
        label={t("settings-appearance-zoom")}
        value={String(a.zoomPct)}
        anchor="appearance.zoom"
        options={ZOOM_STEPS.map((z) => ({ value: String(z), label: `${z} %` }))}
        onChange={(v) => void patch({ appearance: { zoomPct: Number(v) } })}
      />
      <RadioGroup
        label={t("settings-appearance-density")}
        value={a.density}
        anchor="appearance.density"
        orientation="horizontal"
        options={[
          { value: "compact", label: t("settings-appearance-density-compact", { px: SIZES.rowHeight.compact }) },
          { value: "normal", label: t("settings-appearance-density-normal", { px: SIZES.rowHeight.normal }) },
        ]}
        onChange={(v) => void patch({ appearance: { rowDensity: v } })}
      />
      <Switch isSelected={a.zebra} anchor="appearance.zebra" onChange={(v) => void patch({ appearance: { zebra: v } })}>
        {t("settings-appearance-zebra")}
      </Switch>
      <LiveInterval />
      <div className="os-settings-actions">
        <Button onPress={() => void reset()}>{t("settings-appearance-reset")}</Button>
      </div>
    </div>
  );
}

export const section: SettingsSection = { id: "appearance", order: 20, titleKey: sectionTitleKey("appearance"), Component: Appearance };
