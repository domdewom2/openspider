import { getPath } from "../../frame/model";
import { useT } from "../../../../i18n";
import { useConfigValue } from "../../frame/context";
import { definePage, f } from "../../frame/definePage";
import { FieldFrame } from "../../frame/fields";
import { Action, Input, RowActions, RowIssues, coreGuard, moveRow } from "../custom-search/shared";
export interface PositionRule { position: string; contains: string[] }
export const DEFAULT_RULES: PositionRule[] = [{ position: "Navigation", contains: ["nav", "menu"] }, { position: "Header", contains: ["header"] }, { position: "Aside", contains: ["aside"] }, { position: "Sidebar", contains: ["sidebar"] }, { position: "Footer", contains: ["footer"] }, { position: "Content", contains: [] }];
function Positions() {
  const t = useT(); const { value = [], set, lock } = useConfigValue<PositionRule[]>("link_positions.rules"); const disabled = !!lock;
  const update = (i: number, p: Partial<PositionRule>) => set(value.map((r, n) => n === i ? { ...r, ...p } : r));
  return <FieldFrame path="link_positions.rules" showIssues={false}><p className="os-hint">{t("fach-position-hint")}</p>{value.map((r, i) => <fieldset key={i} className="os-cfg-group"><legend>{r.position}</legend>
    <Input label={t("fach-position")} value={r.position} onChange={position => update(i, { position })} disabled={disabled || i === value.length - 1} />
    <Input multiline label={t("fach-substrings")} value={r.contains.join("\n")} onChange={v => update(i, { contains: v.split(/\r?\n/) })} disabled={disabled || i === value.length - 1} />
    <RowIssues path={`link_positions.rules[${i}]`} />
    <RowActions index={i} count={value.length} disabled={disabled} lastFixed remove={() => set(value.filter((_, n) => n !== i))} move={to => set(moveRow(value, i, to))} />
  </fieldset>)}<Action disabled={disabled} onClick={() => set([...value.slice(0, -1), { position: "", contains: [""] }, value.at(-1) ?? { position: "Content", contains: [] }])}>{t("fach-add-position")}</Action>
  <Action disabled={disabled} onClick={() => set(structuredClone(DEFAULT_RULES))}>{t("fach-reset")}</Action><RowIssues path="link_positions.rules" exact /></FieldFrame>;
}
export const page = definePage({ id: "link-positions", group: "custom", order: 130, anchors: ["link_positions"], fields: [f("link_positions.rules", { hidden: true, labelKey: "fach-positions" })], After: Positions, saveGuard: coreGuard("link_positions.rules", d => JSON.stringify(d.get("link_positions.rules")) !== JSON.stringify(getPath(d.saved, "link_positions.rules"))) });
