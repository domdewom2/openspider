import { useState } from "react";
import { useT } from "../../../i18n";
import { SearchField, Select } from "../../../ui/primitives/fields";
import type { SortKey } from "../../../ipc/gen/SortKey";
import { TablePanel } from "../frame/TablePanel";
import { LINK_COLUMNS } from "../anchors/model";
import type { DetailPanelProps } from "../frame/types";
export const RESOURCE_COLUMNS = [...LINK_COLUMNS.filter(c => c !== "Original URL"), "Content Type", "Size", "Blocked By"];
/** UI-036 columns the core's resource view does not offer yet: it knows the link columns and `Size (Bytes)`. Asking for
 *  an unknown column fails with err-unknown-column, so the panel retries with this list instead of showing an error (A01-12). */
export const RESOURCE_FALLBACK_COLUMNS = [...LINK_COLUMNS.filter(c => c !== "Original URL"), "Size (Bytes)"];
export function Resources(props: DetailPanelProps) {
  const t = useT(); const [filter, setFilter] = useState("All"); const [search, setSearch] = useState(""); const [sort, setSort] = useState<SortKey[]>([]);
  return <><div className="os-detail-toolbar"><Select label={t("detail-filter")} value={filter} onChange={setFilter} options={["All", "JavaScript", "CSS", "Images", "Fonts", "Iframes", "Media", "Other", "Blocked"].map(value => ({ value, label: value === "All" || value === "Blocked" ? t(`detail-filter-${value}`) : value }))} />
    <SearchField label={t("detail-search")} value={search} onChange={setSearch} clearLabel={t("detail-clear")} /></div>
    <TablePanel {...props} title={t("detail-tab-resources")} columns={RESOURCE_COLUMNS} fallbackColumns={RESOURCE_FALLBACK_COLUMNS} spec={{ kind: "resources", urlIds: props.selection.urlIds, filter: { id: filter }, search, sort }} onSort={setSort} /></>;
}
