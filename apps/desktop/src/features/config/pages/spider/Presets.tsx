// Spider › Voreinstellungen (CRAWL-140, F-13): Schwellwerte der Issue-Filter. Sie wirken ohne Neu-Crawl und bleiben auch
// während „Läuft“ editierbar; „Zurücksetzen“ stellt die Werkseinstellung je Gruppe wieder her.
import { useT } from "../../../../i18n";
import { Banner, Button } from "../../../../ui/primitives";
import { useConfig } from "../../frame/context";
import { ConfigField, PageHeading } from "../../frame/fields";
import { f } from "../../frame/definePage";
import { defaultResolver } from "../../frame/model";
import { pageTitleKey, type FieldDef } from "../../frame/types";

interface Group {
  id: string;
  fields: FieldDef[];
}

export const PRESET_GROUPS: Group[] = [
  { id: "title", fields: ["title.max_chars", "title.min_chars", "title.max_px", "title.min_px"].map((p) => f(p)) },
  { id: "meta_description", fields: ["meta_description.max_chars", "meta_description.min_chars", "meta_description.max_px", "meta_description.min_px"].map((p) => f(p)) },
  { id: "url", fields: [f("url.max_chars"), f("url.internal_search_params", { kind: "lines" }), f("url.internal_search_segments", { kind: "lines" })] },
  { id: "h1", fields: [f("h1.max_chars")] },
  { id: "h2", fields: [f("h2.max_chars")] },
  { id: "image", fields: [f("image.max_kb", { unitKey: "config-unit-kb" }), f("image.alt_max_chars")] },
  { id: "content", fields: [f("content.low_word_count"), f("content.soft404_phrases", { kind: "lines" })] },
  { id: "links", fields: [f("links.high_crawl_depth"), f("links.high_internal_outlinks"), f("links.high_external_outlinks"), f("links.non_descriptive_anchors", { kind: "lines" })] },
  { id: "validation", fields: [f("validation.html_doc_max_bytes", { unitKey: "config-unit-bytes" }), f("validation.resource_max_bytes", { unitKey: "config-unit-bytes" })] },
  {
    id: "duplicates",
    fields: [f("duplicates.exact_enabled"), f("duplicates.near_enabled"), f("duplicates.near_threshold_pct", { unitKey: "config-unit-pct" }), f("duplicates.only_indexable"), f("duplicates.ignore_paginated")],
  },
  { id: "issues", fields: [f("issues.ignore_non_indexable")] },
];

export const presetFields: FieldDef[] = PRESET_GROUPS.flatMap((g) => g.fields);

export function Presets() {
  const t = useT();
  const { draft, runMode } = useConfig();
  const standard = defaultResolver(draft.saved, draft.defaultRows);
  const reset = (g: Group) => {
    for (const def of g.fields) draft.set(def.path, standard(def.path) ?? null);
  };
  return (
    <section className="os-cfg-page" aria-label={t(pageTitleKey("spider-presets"))} data-page-id="spider-presets">
      <PageHeading titleKey={pageTitleKey("spider-presets")} descriptionKey="config-presets-desc" />
      {runMode === "running" && <Banner kind="info">{t("config-presets-live")}</Banner>}
      {PRESET_GROUPS.map((g) => (
        <fieldset key={g.id} className="os-cfg-group" data-group={g.id}>
          <legend>{t(`config-preset-${g.id.replace(/_/g, "-")}`)}</legend>
          {g.fields.map((def) => (
            <div key={def.path} className="os-cfg-row">
              <ConfigField def={def} />
            </div>
          ))}
          <Button variant="secondary" onPress={() => reset(g)} isDisabled={runMode === "foreign"} aria-label={t("config-preset-reset-group", { group: t(`config-preset-${g.id.replace(/_/g, "-")}`) })}>
            {t("config-preset-reset")}
          </Button>
        </fieldset>
      ))}
    </section>
  );
}
