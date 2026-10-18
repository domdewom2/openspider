import { create } from "zustand";
import type { ScopeArgs } from "../../../ipc/gen/ScopeArgs";
import type { ViewSpec } from "../../../ipc/gen/ViewSpec";

export interface TableNavigation {
  spec: Extract<ViewSpec, { kind: "tab" }>;
  scrollToTop: true;
  focus: boolean;
  revision: number;
}

/** Shared navigation contract: the table consumes this snapshot, including navigation before it mounts. */
export const useSidebarNavigation = create<{
  scope: ScopeArgs;
  target: TableNavigation | null;
  setScope(scope: ScopeArgs): void;
  navigate(tab: string, filter: string, focus?: boolean): void;
}>((set, get) => ({
  scope: {},
  target: null,
  setScope(scope) {
    const target = get().target;
    set({ scope });
    if (target) get().navigate(target.spec.tab, target.spec.filter);
  },
  navigate(tab, filter, focus = false) {
    const target: TableNavigation = {
      spec: { kind: "tab", tab, filter, sort: [], ...get().scope },
      scrollToTop: true,
      focus,
      revision: (get().target?.revision ?? 0) + 1,
    };
    set({ target });
    document.dispatchEvent(new CustomEvent("os:table-navigate", { detail: target }));
    const host = document.querySelector<HTMLElement>('[data-region="table"]');
    host?.scrollTo?.({ top: 0 });
    if (focus && host) {
      const focusable = host.querySelector<HTMLElement>('[role="grid"],button,[tabindex]') ?? host;
      if (focusable === host && !host.hasAttribute("tabindex")) host.tabIndex = -1;
      focusable.focus();
    }
  },
}));
