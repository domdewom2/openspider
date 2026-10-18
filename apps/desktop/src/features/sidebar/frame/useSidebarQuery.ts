import { useQuery } from "@tanstack/react-query";
import { sidebar } from "../../../ipc/sidebar";
import { analysis } from "../../../ipc/analysis";
import { useSession } from "../../../state/session";
import { useSidebarNavigation } from "./state";

/** Versioned keys discard late responses from earlier crawl/scope snapshots; inactive panels do not poll. */
export function useOverview() {
  const scope = useSidebarNavigation(s => s.scope);
  const crawl = useSession(s => s.crawl?.crawlId);
  const counts = useSession(s => s.countsVersion);
  const version = useSession(s => s.sidebarVersion);
  return useQuery({ queryKey: ["sidebar", "overview", crawl, scope, counts, version], queryFn: ({ signal }) => sidebar.overview(scope, signal), enabled: !!crawl, gcTime: 0 });
}

export function useAnalysis() {
  const crawl = useSession(s => s.crawl?.crawlId);
  const snapshot = useSession(s => s.analysis);
  const query = useQuery({ queryKey: ["sidebar", "analysis", crawl], queryFn: ({ signal }) => analysis.status(signal), enabled: !!crawl, gcTime: 0 });
  return { ...query, data: crawl ? snapshot ?? query.data : undefined };
}
