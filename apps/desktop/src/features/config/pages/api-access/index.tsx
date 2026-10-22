import { useT } from "../../../../i18n";
import { definePage } from "../../frame/definePage";
import { Action } from "../custom-search/shared";
import { useApiSources } from "../../../integrations/useApiSources";
import { openApiAccess } from "../../../integrations/store";
import { messageText } from "../../frame/context";
function Access() {
  const t = useT(); const conn = useApiSources();
  return <><p className="os-hint">{t("fach-api-hint")}</p>{conn.error && <p role="alert" className="os-error">{messageText(t, conn.error.messageKey, conn.error.args, conn.error.detail)}</p>}
    <ul>{conn.sources?.map(source => <li key={source.kind}><span>{t(source.labelKey)} · {t(`fach-state-${conn.stateOf(source.kind)}`)}</span> <Action onClick={() => { openApiAccess(source.kind); }}>{t("fach-api-open")}</Action></li>)}</ul>
    <p className="os-hint">{t("fach-api-draft")}</p><Action onClick={() => { openApiAccess(); }}>{t("fach-api-open")}</Action></>;
}
export const page = definePage({ id: "api", order: 140, anchors: ["api", "integrations"], fields: [], After: Access });
