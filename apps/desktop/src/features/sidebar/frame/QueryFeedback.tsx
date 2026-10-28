import { Button } from "../../../ui/primitives/Button";
import { useT } from "../../../i18n";
import { toCmdError } from "../../../ipc/errors";

export function QueryFeedback({ error, loading, retry }: { error?: unknown; loading?: boolean; retry?: () => void }) {
  const t = useT();
  if (error) {
    const e = toCmdError(error);
    return <div role="alert">{t(e.messageKey, e.args as Record<string, string>)} {retry && <Button onPress={retry}>{t("sidebar-retry")}</Button>}</div>;
  }
  return loading ? <p role="status">{t("sidebar-loading")}</p> : null;
}
