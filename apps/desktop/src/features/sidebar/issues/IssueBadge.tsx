import type { IssueBadge as Badge } from "../../../ipc/gen/IssueBadge";
import { useT } from "../../../i18n";
import { IssueTypeIcon } from "../../../ui/icons";

export function IssueBadge({ badge }: { badge: Badge }) {
  const t = useT();
  return <span className={`os-sidebar-badge os-sidebar-priority-${badge.priority}`}><IssueTypeIcon type={badge.issueType} /><span>{t(`sidebar-type-${badge.issueType}`)} · {t(`sidebar-priority-${badge.priority}`)}</span></span>;
}
