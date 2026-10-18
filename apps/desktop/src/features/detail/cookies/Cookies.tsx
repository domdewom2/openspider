import { useState } from "react";
import { useT } from "../../../i18n";
import type { SortKey } from "../../../ipc/gen/SortKey";
import { TablePanel } from "../frame/TablePanel";
import { Banner } from "../../../ui/primitives/Feedback";
import { SearchField } from "../../../ui/primitives/fields";
import type { DetailPanelProps } from "../frame/types";
export const COOKIE_COLUMNS = ["Address", "Cookie Name", "Cookie Value", "Domain", "Path", "Expires", "Max-Age", "Secure", "HttpOnly", "SameSite", "Type", "Session", "First-Party"];
export const COOKIE_HIDDEN = ["Path", "Max-Age"];
export function Cookies(props: DetailPanelProps) {
  const t = useT(); const [search, setSearch] = useState(""); const [sort, setSort] = useState<SortKey[]>([]);
  const focus = props.selection.focus ?? props.selection.urlIds[0];
  return <>{!props.rendering && <Banner>{t("detail-cookies-js")}</Banner>}
    <SearchField label={t("detail-search")} value={search} onChange={setSearch} clearLabel={t("detail-clear")} />
    <TablePanel {...props} title={t("detail-tab-cookies")} columns={COOKIE_COLUMNS} hiddenColumns={COOKIE_HIDDEN}
      spec={{ kind: "detail", detail: "cookies", urlIds: [focus], params: { search }, sort }} onSort={setSort} /></>;
}
