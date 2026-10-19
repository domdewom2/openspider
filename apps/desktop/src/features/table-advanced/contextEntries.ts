// [P1] entries of the upper table's context menu (UI-110): screenshot, visualisations, crawl path, check with…, segment actions.
import type { useT } from "../../i18n";
import type { VizKind } from "../../ipc/gen/VizKind";
import type { MenuEntry } from "../../ui/primitives/Menu";

/** Actions other features register (detail, dialogs); unregistered ones stay disabled with a reason. */
export const URL_ACTIONS = { screenshot: "url.screenshot", crawlPath: "url.crawlPath", checkWith: "url.checkWith", jumpSegment: "segment.jump" } as const;
export const VIZ_KINDS: VizKind[] = ["crawlTree", "crawlGraph", "directoryTree", "directoryGraph", "crawlGraph3d", "directoryGraph3d", "anchorWords", "bodyWords"];

export function advancedContextEntries(t: ReturnType<typeof useT>, state: { count: number; hasSegments: boolean; hasAction: (id: string) => boolean }, actions: {
  run: (id: string) => void; viz: (kind: VizKind) => void; filterSegment: () => void;
}): MenuEntry[] {
  const single = state.count === 1;
  const reason = (available: boolean) => !state.count ? t("u20-ctx-no-selection") : !single ? t("u20-ctx-single") : !available ? t("u20-ctx-unavailable") : undefined;
  const action = (id: string, key: string, separatorBefore = false): MenuEntry => ({ id, label: t(key), separatorBefore,
    isDisabled: !single || !state.hasAction(id), disabledReason: reason(state.hasAction(id)), onAction: () => actions.run(id) });
  const segmentReason = !state.hasSegments ? t("u20-ctx-no-segments") : reason(true);
  return [
    action(URL_ACTIONS.screenshot, "u20-ctx-screenshot", true),
    ...VIZ_KINDS.map((kind, i): MenuEntry => ({ id: `viz-${kind}`, label: t(`u20-viz-${kind}`), separatorBefore: i === 0,
      isDisabled: !single, disabledReason: reason(true), onAction: () => actions.viz(kind) })),
    action(URL_ACTIONS.crawlPath, "u20-ctx-crawl-path", true),
    action(URL_ACTIONS.checkWith, "u20-ctx-check-with"),
    ...(state.hasSegments ? [
      { id: "segment-filter", label: t("u20-ctx-segment-filter"), separatorBefore: true, isDisabled: !single, disabledReason: segmentReason, onAction: actions.filterSegment },
      { ...action(URL_ACTIONS.jumpSegment, "u20-ctx-segment-jump") },
    ] : [{ id: "segment-filter", label: t("u20-ctx-segment-filter"), separatorBefore: true, isDisabled: true, disabledReason: segmentReason }]),
  ];
}
