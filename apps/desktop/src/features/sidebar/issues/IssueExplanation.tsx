import { useQuery } from "@tanstack/react-query";
import { useLanguageStore, useT } from "../../../i18n";
import { sidebar } from "../../../ipc/sidebar";
import type { IssueRow } from "../../../ipc/gen/IssueRow";
import { useSession } from "../../../state/session";
import { IssueBadge } from "./IssueBadge";
import { QueryFeedback } from "../frame/QueryFeedback";

export function documentationUrl(value?: string | null): string | null {
  if (!value) return null;
  try { const url = new URL(value); return url.protocol === "https:" ? url.href : null; } catch { return null; }
}

/** Text is plain React text from the core; no Markdown/HTML or secondary issue text resources. */
export function IssueExplanation({ issue }: { issue: IssueRow }) {
  const t = useT();
  const lang = useLanguageStore(s => s.lang);
  const version = useSession(s => s.issuesVersion);
  const crawl = useSession(s => s.crawl?.crawlId);
  const query = useQuery({ queryKey: ["sidebar", "explain", crawl, issue.id, lang, version], queryFn: ({ signal }) => sidebar.issueExplain({ issueId: issue.id, lang }, signal), gcTime: 0 });
  const url = documentationUrl(query.data?.learnMoreUrl);
  return <section className="os-sidebar-explanation" aria-label={t("sidebar-explanation")}>
    <h3>{issue.tab}: {issue.filter}</h3><IssueBadge badge={{ issueType: issue.type, priority: issue.priority }} />
    <QueryFeedback error={query.error} loading={query.isPending} retry={() => void query.refetch()} />
    {query.data && <><dl>{(["what", "why", "fix"] as const).map(key => <div key={key}><dt>{t(`sidebar-${key}`)}</dt><dd>{query.data[key]}</dd></div>)}</dl>{query.data.rule && <p>{t("sidebar-rule")}: {query.data.rule}</p>}{url && <a href={url} target="_blank" rel="noopener noreferrer">{t("sidebar-learn-more")}</a>}</>}
  </section>;
}
