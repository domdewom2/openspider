import { useState } from "react";
import { useT } from "../../i18n";
import { Checkbox, Select, Tabs, TextField } from "../../ui/primitives";
import type { SitemapOptions as Options } from "./sitemap";

export function SitemapOptions({ value, onChange, listMode, hreflangAvailable }: {
  value: Options; onChange(value: Options): void; listMode: boolean; hreflangAvailable: boolean;
}) {
  const t = useT();
  const [tab, setTab] = useState("pages");
  const [invalidPriorities, setInvalidPriorities] = useState<Record<number, string>>({});
  const fields = ["include_noindex", "include_canonicalised", "include_paginated", "include_pdfs", "include_3xx", "include_4xx", "include_5xx", "include_no_response"] as const;
  const frequencies = ["always", "hourly", "daily", "weekly", "monthly", "yearly", "never"] as const;
  return <Tabs label={t("export-sitemap-options")} selected={tab} onSelect={setTab} tabs={[
    { id: "pages", label: t("export-pages"), content: <div className="os-export-fields">
      {fields.map(key => <Checkbox key={key} isSelected={value[key]} onChange={v => onChange({ ...value, [key]: v })}>{t(`export-${key.replaceAll("_", "-")}`)}</Checkbox>)}
      <TextField label={t("export-exclusions")} value={value.exclude_patterns.join("\n")} onChange={v => onChange({ ...value, exclude_patterns: v.split("\n").filter(Boolean) })} />
    </div> },
    { id: "lastmod", label: t("export-lastmod"), content: <div className="os-export-fields">
      <Checkbox isSelected={value.lastmod.enabled} onChange={enabled => onChange({ ...value, lastmod: { ...value.lastmod, enabled } })}>{t("export-lastmod")}</Checkbox>
      <Select label={t("export-source")} value={value.lastmod.source} options={[{ value: "server", label: t("export-server") }, { value: "custom", label: t("export-custom") }]} onChange={source => onChange({ ...value, lastmod: { ...value.lastmod, source: source as "server" | "custom" } })} />
      {value.lastmod.source === "custom" && <label>{t("export-date")}<input type="date" value={value.lastmod.custom_date ?? ""} onChange={e => onChange({ ...value, lastmod: { ...value.lastmod, custom_date: e.target.value } })} /></label>}
    </div> },
    { id: "priority", label: t("export-priority"), content: <div className="os-export-fields">
      <Checkbox isSelected={value.priority.enabled} onChange={enabled => onChange({ ...value, priority: { ...value.priority, enabled } })}>{t("export-priority")}</Checkbox>
      {listMode && <p role="note">{t("export-list-depth")}</p>}
      {value.priority.by_depth.map((n, depth) => <label key={depth}>{t("export-depth", { depth: depth === value.priority.by_depth.length - 1 ? `≥ ${depth}` : depth })}
        <input type="number" min="0" max="1" step="0.1" disabled={!value.priority.enabled} value={invalidPriorities[depth] ?? (Number.isFinite(n) ? n : "")}
          aria-invalid={!Number.isFinite(n) || n < 0 || n > 1 || Math.abs(n * 10 - Math.round(n * 10)) > 0.000001}
          onChange={e => {
            const raw = e.target.value;
            setInvalidPriorities(s => ({ ...s, [depth]: raw }));
            onChange({ ...value, priority: { ...value.priority, by_depth: value.priority.by_depth.map((v, i) => i === depth ? raw === "" ? NaN : Number(raw) : v) } });
          }} />
      </label>)}
    </div> },
    { id: "changefreq", label: t("export-changefreq"), content: <div className="os-export-fields">
      <Checkbox isSelected={value.changefreq.enabled} onChange={enabled => onChange({ ...value, changefreq: { ...value.changefreq, enabled } })}>{t("export-changefreq")}</Checkbox>
      <Select label={t("export-source")} value={value.changefreq.basis} options={[{ value: "last_modified", label: t("export-server") }, { value: "depth", label: t("export-depth-basis") }]} onChange={basis => onChange({ ...value, changefreq: { ...value.changefreq, basis: basis as "last_modified" | "depth" } })} />
      {value.changefreq.basis === "depth" && value.changefreq.by_depth.map((frequency, depth) => <Select key={depth} label={t("export-depth", { depth: depth === value.changefreq.by_depth.length - 1 ? `≥ ${depth}` : depth })} value={frequency} options={frequencies.map(v => ({ value: v, label: t(`export-frequency-${v}`) }))} onChange={v => onChange({ ...value, changefreq: { ...value.changefreq, by_depth: value.changefreq.by_depth.map((f, i) => i === depth ? v as typeof frequency : f) } })} />)}
    </div> },
    { id: "images", label: t("export-images"), content: <div className="os-export-fields">
      <Checkbox isSelected={value.images.enabled} onChange={enabled => onChange({ ...value, images: { ...value.images, enabled } })}>{t("export-images")}</Checkbox>
      <TextField label={t("export-image-regex")} value={value.images.external_regex.join("\n")} onChange={v => onChange({ ...value, images: { ...value.images, external_regex: v.split("\n").filter(Boolean) } })} />
      <TextField type="number" label={t("export-image-inlinks")} value={value.images.max_inlinks === null ? "" : String(value.images.max_inlinks)} onChange={v => onChange({ ...value, images: { ...value.images, max_inlinks: v === "" ? null : Number(v) } })} />
    </div> },
    { id: "hreflang", label: t("export-hreflang"), content: <div className="os-export-fields">
      {!hreflangAvailable && <p role="note">{t("export-hreflang-required")}</p>}
      <Checkbox isDisabled={!hreflangAvailable} isSelected={hreflangAvailable && value.hreflang.enabled} onChange={enabled => onChange({ ...value, hreflang: { ...value.hreflang, enabled } })}>{t("export-hreflang")}</Checkbox>
      <Checkbox isDisabled={!hreflangAvailable || !value.hreflang.enabled} isSelected={value.hreflang.only_valid_targets} onChange={only_valid_targets => onChange({ ...value, hreflang: { ...value.hreflang, only_valid_targets } })}>{t("export-valid-hreflang")}</Checkbox>
    </div> },
  ]} />;
}
