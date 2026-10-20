import { useState } from "react";
import { useT } from "../../../i18n";
import { Checkbox, SearchField, Select } from "../../../ui/primitives/fields";
import { Button } from "../../../ui/primitives/Button";
import type { SortKey } from "../../../ipc/gen/SortKey";
import { TablePanel } from "../frame/TablePanel";
import { DEFAULT_LINK_OPTIONS, linkColumns, linkSpec } from "../anchors/model";
import type { DetailPanelProps } from "../frame/types";

const NO_ORIGIN = ["Link Origin"];
export function Links(props: DetailPanelProps & { direction: "inlinks" | "outlinks" }) {
  const { direction } = props; const t = useT();
  const [options, setOptions] = useState(DEFAULT_LINK_OPTIONS); const [search, setSearch] = useState(""); const [sort, setSort] = useState<SortKey[]>([]);
  const spec = linkSpec(direction, props.selection.urlIds, options, search, sort);
  const filters = ["All", "Hyperlinks", ...(direction === "outlinks" ? ["Internal", "External", ...(props.storeUncrawlable ? ["Uncrawlable"] : [])] : [])];
  return <><div className="os-detail-toolbar">
    <Select label={t("detail-view")} value={options.mode} onChange={value => { setOptions(o => ({ ...o, mode: value as "links" | "anchors", excludeSelf: value === "anchors", anchor: undefined })); setSort([]); }} options={[{ value: "links", label: t("detail-links") }, { value: "anchors", label: "Anchors" }]} />
    <Select label={t("detail-filter")} value={options.id} onChange={id => setOptions(o => ({ ...o, id }))} options={filters.map(value => ({ value, label: t(`detail-filter-${value}`) }))} />
    <Select label={t("detail-position")} value={options.position} onChange={position => setOptions(o => ({ ...o, position }))} options={[{ value: "", label: t("detail-filter-All") }, ...props.positions.map(value => ({ value, label: value }))]} />
    <Checkbox isSelected={options.excludeSelf} onChange={excludeSelf => setOptions(o => ({ ...o, excludeSelf }))}>{t("detail-exclude-self")}</Checkbox>
    <Checkbox isSelected={options.excludeNofollow} onChange={excludeNofollow => setOptions(o => ({ ...o, excludeNofollow }))}>{t("detail-exclude-nofollow")}</Checkbox>
    <SearchField label={t("detail-search")} value={search} onChange={setSearch} clearLabel={t("detail-clear")} />
    {options.anchor !== undefined && <Button onPress={() => setOptions(o => ({ ...o, anchor: undefined }))}>{t("detail-clear-anchor", { anchor: options.anchor || t("detail-empty-anchor") })}</Button>}
  </div><TablePanel {...props} title={t(`detail-tab-${direction}`)} spec={spec} columns={linkColumns(direction, options.mode)} hiddenColumns={!props.rendering ? NO_ORIGIN : undefined} onSort={setSort}
    onSelectRow={options.mode === "anchors" ? get => { setOptions(o => ({ ...o, mode: "links", anchor: get("Anchor Text") })); setSort([]); } : undefined}
    onActivateRow={get => {
      if (options.mode === "anchors") { setOptions(o => ({ ...o, mode: "links", anchor: get("Anchor Text") })); setSort([]); }
      else { const address = get(direction === "inlinks" ? "From" : "To"); if (address) props.actions.navigate({ address, tab: "auto" }); }
    }} /></>;
}
export function Inlinks(props: DetailPanelProps) { return <Links {...props} direction="inlinks" />; }
