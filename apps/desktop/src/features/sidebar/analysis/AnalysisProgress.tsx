import { useT } from "../../../i18n";
import { useAnalysis } from "../frame/useSidebarQuery";

/** Compact status-bar contribution, kept alive when the sidebar or Analysis tab is hidden. */
export function AnalysisProgress() {
  const t = useT();
  const { data } = useAnalysis();
  if (!data?.running) return null;
  return <span className="os-sidebar-progress" role="status">{t("sidebar-analysis-running")}: {data.current ? t(`sidebar-step-${data.current}`, undefined, { fallback: data.current }) : t("sidebar-analysis")}{data.percent == null ? "" : ` · ${Math.round(data.percent)} %`}</span>;
}
