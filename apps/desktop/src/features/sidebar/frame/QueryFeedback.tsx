import { useT } from "../../../i18n";
import { toCmdError } from "../../../ipc/errors";

export function QueryFeedback({ error, loading, retry }: { error?: unknown; loading?: boolean; retry?: () => void }) {
  const t = useT();
  if (error) {
    const e = toCmdError(error);
    return <div role="alert">{t(e.messageKey, e.args as Record<string, string>)} {retry && <button type="button" onClick={retry}>{t("sidebar-retry")}</button>}</div>;
  }
  return loading ? <p role="status">{t("sidebar-loading")}</p> : null;
}
