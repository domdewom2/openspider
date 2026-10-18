import type { FilterStateDto } from "../../../ipc/gen/FilterStateDto";
import { useT } from "../../../i18n";
import { openSettingsWindow } from "../../../app/windows";

export function hasCount(state: FilterStateDto): boolean {
  return state.kind === "ready" || state.kind === "stale" || state.kind === "incomplete";
}

export function FilterStatus({ state, startAnalysis }: { state: FilterStateDto; startAnalysis?: () => void }) {
  const t = useT();
  if (state.kind === "ready") return null;
  const label = t(`sidebar-filter-${state.kind}`, state.kind === "incomplete" ? { count: state.unchecked } : undefined);
  if (state.kind === "needsCrawlAnalysis" && startAnalysis) return <button type="button" onClick={startAnalysis}>{label}</button>;
  if (state.kind === "needsConfig") return <button type="button" onClick={() => void openSettingsWindow(state.key)}>{label}</button>;
  if (state.kind === "needsRendering") return <button type="button" onClick={() => void openSettingsWindow("rendering")}>{label}</button>;
  return <span className="os-sidebar-state">{label}</span>;
}
