import { Button } from "../../../ui/primitives/Button";
import { useState } from "react";
import { detail } from "../../../ipc/detail";
import { useT } from "../../../i18n";
import { SearchField } from "../../../ui/primitives/fields";
import { ContextMenu } from "../../../ui/primitives/ContextMenu";
import { useDetailRequest } from "../frame/request";
import { DetailEmpty, DetailError, DetailLoading } from "../frame/Feedback";
import { toCmdError } from "../../../ipc/errors";
import { EMPTY_CODES, type EmptyState } from "../../../ui/emptyState/codes";
import type { DetailPanelProps } from "../frame/types";

export const HEADER_PREVIEW_BYTES = 8192;
export function headerPreview(value: string): string {
  if (new TextEncoder().encode(value).length <= HEADER_PREVIEW_BYTES) return value;
  let text = ""; let bytes = 0;
  for (const char of value) { bytes += new TextEncoder().encode(char).length; if (bytes > HEADER_PREVIEW_BYTES) break; text += char; }
  return `${text}…`;
}
/** The core answers a missing prerequisite with `err-detail-unavailable` and the empty state in `args.state` (UI-016). */
export function unavailableState(error: unknown): EmptyState | null {
  const state = toCmdError(error).args.state as Partial<EmptyState> | undefined;
  if (!state || typeof state !== "object" || !(EMPTY_CODES as readonly string[]).includes(state.code as string)) return null;
  // The core names the setting by its configuration path (`extraction.http_headers`); the texts are keyed by the short name.
  return { ...state, ...(state.arg ? { arg: state.arg.replace(/^extraction\./, "") } : {}) } as EmptyState;
}
export function Headers({ selection, actions }: DetailPanelProps) {
  const t = useT(); const [search, setSearch] = useState("");
  const focus = selection.focus ?? selection.urlIds[0];
  const result = useDetailRequest(`headers:${selection.crawlId}:${focus}`, signal => detail.headers({ urlId: focus }, signal));
  if (result.error) { const state = unavailableState(result.error); return state ? <DetailEmpty state={state} /> : <DetailError error={result.error} />; }
  if (!result.value) return <DetailLoading />;
  const needle = search.toLocaleLowerCase();
  return <><Button onPress={() => actions.export({ kind: "detail", detail: "headers", urlIds: [focus], params: { search }, sort: [] }, { kind: "ids", ids: [focus] })}>{t("detail-export")}</Button><SearchField label={t("detail-search")} value={search} onChange={setSearch} clearLabel={t("detail-clear")} />
    <p>{result.value.statusLine}</p><div className="os-detail-headers os-detail-scroll">{(["request", "response"] as const).map(side => <section key={side} aria-label={t(`detail-${side}`)}>
      <h3>{t(`detail-${side}`)}</h3><table className="os-proptable"><thead><tr><th>Header Name</th><th>Header Value</th></tr></thead><tbody>
        {result.value![side].filter(h => `${h.name}\n${h.value}`.toLocaleLowerCase().includes(needle)).map((header, index) => <tr key={index}><th scope="row">{header.name}</th><td>
          <ContextMenu label={t("detail-context")} entries={[{ id: "copy", label: t("detail-copy-value"), onAction: () => { void navigator.clipboard.writeText(header.value).catch(actions.reportError); } }]}>
            <span tabIndex={0}>{headerPreview(header.value)}</span>
          </ContextMenu></td></tr>)}
      </tbody></table></section>)}</div></>;
}
