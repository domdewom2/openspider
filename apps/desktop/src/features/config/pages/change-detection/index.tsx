import { useEffect, useState } from "react";
import { useT } from "../../../../i18n";
import { compare } from "../../../../ipc/compare";
import { useConfigValue } from "../../frame/context";
import { definePage, f } from "../../frame/definePage";
import { FieldFrame } from "../../frame/fields";
export const ELEMENTS = ["indexability", "page_titles", "meta_description", "h1", "word_count", "crawl_depth", "inlinks", "unique_inlinks", "internal_outlinks", "unique_internal_outlinks", "external_outlinks", "unique_external_outlinks", "structured_data_unique_types", "content"] as const;
function Elements() {
  const t = useT(); const { value = [], set, lock } = useConfigValue<string[]>("compare.change_detection.elements");
  const [available, setAvailable] = useState(false);
  useEffect(() => { let live = true; void compare.contentAvailable().then(v => { if (live) setAvailable(v === true); }).catch(() => { if (live) setAvailable(false); }); return () => { live = false; }; }, []);
  return <FieldFrame path="compare.change_detection.elements"><p className="os-hint">{t("fach-change-hint")}</p>{ELEMENTS.map(element => <label className="os-cfg-field" key={element} title={element === "content" && !available ? t("fach-content-unavailable") : undefined}><span><input type="checkbox" checked={value.includes(element)} disabled={!!lock || (element === "content" && !available)} onChange={e => set(ELEMENTS.filter(v => v === element ? e.target.checked : value.includes(v)))} />{t(`fach-element-${element.replaceAll("_", "-")}`)}</span></label>)}{!available && <p className="os-hint">{t("fach-content-unavailable")}</p>}</FieldFrame>;
}
export const page = definePage({ id: "change-detection", order: 180, anchors: ["compare.change_detection", "change-detection"], fields: [f("compare.change_detection.elements", { hidden: true, labelKey: "fach-change-elements" }), f("compare.change_detection.content_threshold_pct", { labelKey: "fach-content-threshold" })], Before: Elements });
