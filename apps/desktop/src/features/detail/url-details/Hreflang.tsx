import { detail } from "../../../ipc/detail";
import { Banner } from "../../../ui/primitives/Feedback";
import { useDetailRequest } from "../frame/request";
import { useState } from "react";
import { useT } from "../../../i18n";
import type { SortKey } from "../../../ipc/gen/SortKey";
import { TablePanel } from "../frame/TablePanel";
import type { DetailPanelProps } from "../frame/types";
export const HREFLANG_COLUMNS = ["Source", "Language Code", "Hreflang URL", "Status Code", "Indexability", "Confirmation"];
export function Hreflang(props: DetailPanelProps) {
  const t = useT(); const [sort, setSort] = useState<SortKey[]>([]);
  const truncation = useDetailRequest(`hreflang-counts:${props.selection.crawlId}:${props.selection.urlIds.join(",")}`, async signal => {
    let total = 0;
    for (let offset = 0; offset < props.selection.urlIds.length && !signal.aborted; offset += 8) {
      const rows = await Promise.all(props.selection.urlIds.slice(offset, offset + 8).map(urlId => detail.urlDetails({ urlId }, signal)));
      total += rows.reduce((sum, fields) => sum + Number(fields.find(row => row.name === "Hreflang Truncated")?.value ?? 0), 0);
    }
    return total;
  });
  return <>{!!truncation.value && <Banner kind="warning">{t("detail-hreflang-truncated", { count: truncation.value })}</Banner>}<TablePanel {...props} title={t("detail-tab-urlDetails")} columns={props.selection.urlIds.length > 1 ? ["Address", ...HREFLANG_COLUMNS] : HREFLANG_COLUMNS}
    spec={{ kind: "detail", detail: "hreflang", urlIds: props.selection.urlIds, params: {}, sort }} onSort={setSort} /></>;
}
