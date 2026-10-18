import { useState } from "react";
import { useT } from "../../../i18n";
import { SearchField, Select } from "../../../ui/primitives/fields";
import type { SortKey } from "../../../ipc/gen/SortKey";
import { TablePanel } from "../frame/TablePanel";
import { LINK_COLUMNS } from "../anchors/model";
import type { DetailPanelProps } from "../frame/types";
export const RESOURCE_COLUMNS = [...LINK_COLUMNS.filter(c => c !== "Original URL"), "Content Type", "Size", "Blocked By"];
export function Resources(props: DetailPanelProps) {
  const t = useT(); const [filter, setFilter] = useState("All"); const [search, setSearch] = useState(""); const [sort, setSort] = useState<SortKey[]>([]);
  return <><div className="os-detail-toolbar"><Select label={t("detail-filter")} value={filter} onChange={setFilter} options={["All", "JavaScript", "CSS", "Images", "Fonts", "Iframes", "Media", "Other", "Blocked"].map(value => ({ value, label: value === "All" || value === "Blocked" ? t(`detail-filter-${value}`) : value }))} />
    <SearchField label={t("detail-search")} value={search} onChange={setSearch} clearLabel={t("detail-clear")} /></div>
    <TablePanel {...props} title={t("detail-tab-resources")} columns={RESOURCE_COLUMNS} spec={{ kind: "resources", urlIds: props.selection.urlIds, filter: { id: filter }, search, sort }} onSort={setSort} /></>;
}
