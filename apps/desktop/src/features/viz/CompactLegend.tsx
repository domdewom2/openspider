// Kompakte Legende der Farbgebung auf der Zeichenfläche (M3-V02, Optik): nur die Farben des gewählten Farbmodus und
// eingeblendete Linkarten; die ausführliche Legende mit Skalen bleibt in der Seitenleiste.
import type { ReactNode } from "react";
import { useT } from "../../i18n";
import type { ColorMode, PaletteColors } from "./model/encoding";

export interface CompactLegendProps {
  colorMode: ColorMode;
  colors: PaletteColors;
  segments?: readonly string[];
  directory: boolean;
  links?: { inlink: boolean; outlink: boolean; inColor: string; outColor: string } | null;
}

function Dot({ color, hollow }: { color: string; hollow?: boolean }) {
  return (
    <svg width="10" height="10" aria-hidden>
      <circle cx="5" cy="5" r="4" fill={hollow ? "none" : color} stroke={color} strokeWidth="1.5" />
    </svg>
  );
}

function Line({ color, dashed }: { color: string; dashed?: boolean }) {
  return (
    <svg width="14" height="10" aria-hidden>
      <line x1="1" y1="5" x2="13" y2="5" stroke={color} strokeWidth="2" strokeDasharray={dashed ? "3 2" : undefined} />
    </svg>
  );
}

export function CompactLegend({ colorMode, colors, segments = [], directory, links }: CompactLegendProps) {
  const t = useT();
  const items: { key: string; mark: ReactNode; label: string }[] = [];
  if (colorMode === "indexability") {
    items.push({ key: "i", mark: <Dot color={colors.indexable} />, label: t("viz-legend-indexable") });
    items.push({ key: "n", mark: <Dot color={colors.nonIndexable} />, label: t("viz-legend-non-indexable") });
  } else if (colorMode === "statusClass") {
    for (const c of ["2xx", "3xx", "4xx", "5xx"] as const) items.push({ key: c, mark: <Dot color={colors.status[c]} />, label: c });
  } else if (colorMode === "segment") {
    segments.slice(0, 4).forEach((c, i) => items.push({ key: `s${i}`, mark: <Dot color={c} />, label: t("viz-legend-segment-n", { n: i + 1 }) }));
    items.push({ key: "s-", mark: <Dot color={colors.noData} />, label: t("viz-legend-no-segment") });
  } else {
    items.push({ key: "lo", mark: <Dot color={colors.gradient[0]} />, label: t("viz-legend-low") });
    items.push({ key: "hi", mark: <Dot color={colors.gradient[1]} />, label: t("viz-legend-high") });
  }
  if (directory) items.push({ key: "p", mark: <Dot color={colors.path} hollow />, label: t("viz-legend-path-short") });
  if (links?.inlink) items.push({ key: "li", mark: <Line color={links.inColor} />, label: t("viz-legend-inlink") });
  if (links?.outlink) items.push({ key: "lo2", mark: <Line color={links.outColor} dashed />, label: t("viz-legend-outlink") });
  return (
    <ul className="os-viz-legend-compact" aria-label={t("viz-legend-compact")} data-testid="viz-legend-compact">
      {items.map((i) => (
        <li key={i.key}>
          {i.mark}
          <span>{i.label}</span>
        </li>
      ))}
    </ul>
  );
}
