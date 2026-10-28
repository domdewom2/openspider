import { useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { sidebar } from "../../../ipc/sidebar";
import { analysis } from "../../../ipc/analysis";
import { useSession } from "../../../state/session";
import { useSidebarNavigation } from "./state";

/** Keep the last successful snapshot while serial, bundled refreshes catch up. */
export function useOverview() {
  const scope = useSidebarNavigation(s => s.scope);
  const crawl = useSession(s => s.crawl?.crawlId);
  const counts = useSession(s => s.countsVersion);
  const version = useSession(s => s.sidebarVersion);
  const revision = `${counts}:${version}`;
  const latest = useRef(revision);
  latest.current = revision;
  const query = useQuery({ queryKey: ["sidebar", "overview", crawl, scope], queryFn: ({ signal }) => sidebar.overview(scope, signal), enabled: !!crawl, gcTime: 0 });
  const { refetch } = query;
  useEffect(() => {
    if (!crawl) return;
    let requested = revision;
    let pending = false;
    const timer = setInterval(() => {
      if (pending || requested === latest.current) return;
      requested = latest.current;
      pending = true;
      void refetch({ cancelRefetch: false }).finally(() => { pending = false; });
    }, 500);
    return () => clearInterval(timer);
    // Versions are intentionally read through latest: ticks must not cancel requests.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [crawl, scope, refetch]);
  return query;
}

export function useAnalysis() {
  const crawl = useSession(s => s.crawl?.crawlId);
  const snapshot = useSession(s => s.analysis);
  const query = useQuery({ queryKey: ["sidebar", "analysis", crawl], queryFn: ({ signal }) => analysis.status(signal), enabled: !!crawl, gcTime: 0 });
  return { ...query, data: crawl ? snapshot ?? query.data : undefined };
}
