import type { SearchSpec } from "../../../ipc/gen/SearchSpec";
import { useSidebarNavigation, type TableNavigation } from "../frame/state";

/**
 * Sets tab, filter and search of the upper table. `navigate()` of the frame carries no search, so the snapshot is
 * built here with the same contract (scope, scroll to top, optional focus, revision).
 */
export function navigateToTable(tab: string, filter: string, search: SearchSpec, focus = false): void {
  const store = useSidebarNavigation;
  const target: TableNavigation = {
    spec: { kind: "tab", tab, filter, search, sort: [], ...store.getState().scope },
    scrollToTop: true,
    focus,
    revision: (store.getState().target?.revision ?? 0) + 1,
  };
  store.setState({ target });
  document.dispatchEvent(new CustomEvent("os:table-navigate", { detail: target }));
  const host = document.querySelector<HTMLElement>('[data-region="table"]');
  host?.scrollTo?.({ top: 0 });
}
