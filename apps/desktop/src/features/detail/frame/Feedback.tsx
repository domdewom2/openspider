import { useT } from "../../../i18n";
import { toCmdError } from "../../../ipc/errors";
import { Banner, ProgressBar } from "../../../ui/primitives/Feedback";
import { EmptyState } from "../../../ui/primitives/EmptyState";
import { openConfig } from "../../config/frame/store";
import type { EmptyState as EmptyStateDto } from "../../../ipc/gen/EmptyState";

export function DetailLoading() { const t = useT(); return <ProgressBar label={t("detail-loading")} />; }
export function DetailError({ error }: { error: unknown }) {
  const t = useT(); const e = toCmdError(error);
  return <Banner kind="error">{t(`err-code-${e.code.toLowerCase()}`)}</Banner>;
}
export function DetailEmpty({ state }: { state: EmptyStateDto }) {
  return <EmptyState state={state} onAction={action => { if (action.kind === "openSetting") openConfig({ anchor: action.anchor }); }} />;
}
