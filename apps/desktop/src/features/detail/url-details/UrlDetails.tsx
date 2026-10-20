import { detail } from "../../../ipc/detail";
import { useT } from "../../../i18n";
import { ContextMenu } from "../../../ui/primitives/ContextMenu";
import { useDetailRequest } from "../frame/request";
import { DetailError, DetailLoading } from "../frame/Feedback";
import type { DetailPanelProps } from "../frame/types";

export const URL_DETAIL_FIELDS = ["Address", "URL Encoded Address", "Content Type", "Status Code", "Status", "Indexability", "Indexability Status", "Size", "Transferred", "Response Time", "Crawl Depth", "Link Score", "Inlinks", "Unique Inlinks", "Outlinks", "Unique Outlinks", "External Outlinks", "Redirect URL", "Redirect Type", "Canonical Link Element 1", "Meta Robots 1", "X-Robots-Tag 1", "Last Modified", "Crawl Timestamp", "Segment"];
export function displayValue(value: unknown): string { return value === null || value === undefined ? "" : typeof value === "object" ? JSON.stringify(value) : String(value); }
export function UrlDetails({ selection, actions }: DetailPanelProps) {
  const t = useT(); const focus = selection.focus ?? selection.urlIds[0];
  const result = useDetailRequest(`url:${selection.crawlId}:${focus}`, signal => detail.urlDetails({ urlId: focus }, signal));
  if (result.error) return <DetailError error={result.error} />;
  if (!result.value) return <DetailLoading />;
  const values = new Map(result.value.map(row => [row.name, row.value]));
  const fields = [...URL_DETAIL_FIELDS, ...result.value.map(row => row.name).filter(name => !URL_DETAIL_FIELDS.includes(name))];
  const copy = (text: string) => { void navigator.clipboard.writeText(text).catch(actions.reportError); };
  return <div className="os-detail-scroll"><table className="os-proptable"><caption className="os-visually-hidden">{t("detail-tab-urlDetails")}</caption>
    <thead><tr><th>{t("detail-name")}</th><th>{t("detail-value")}</th></tr></thead>
    <tbody>{fields.map(name => { const value = displayValue(values.get(name)); return <tr key={name}><th scope="row">{name}</th><td>
      <ContextMenu label={t("detail-context")} entries={[
        { id: "value", label: t("detail-copy-value"), onAction: () => copy(value) },
        { id: "name-value", label: t("detail-copy-name-value"), onAction: () => copy(`${name}\t${value}`) },
      ]}><span tabIndex={0}>{name === "Redirect URL" && value ? <button className="os-detail-link" onClick={() => actions.navigate({ address: value, tab: "auto" })}>{value}</button> : value || "\u00a0"}</span></ContextMenu>
    </td></tr>; })}</tbody></table></div>;
}
