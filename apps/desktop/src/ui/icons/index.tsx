// Icons: Lucide für Bedienelemente (ISC); fachliche Symbole mit fester Bedeutung als eigene SVGs, deren Pfade das Canvas
// teilt (5.5).
import type { SVGProps } from "react";

export * from "lucide-react";

/** Pfaddaten der Issue-Typ-Symbole (Kreis, Dreieck, Raute) in einer 14×14-Box; im Canvas als `Path2D` zeichenbar. */
export const ISSUE_SHAPES = {
  issue: { circle: { cx: 7, cy: 7, r: 6 }, mark: "M7 3.5v4.2M7 9.6v.9" },
  warning: { path: "M7 1.2 13.2 12.2H0.8Z", mark: "M7 5.2v3.4M7 9.9v.9" },
  opportunity: { path: "M7 0.8 13.2 7 7 13.2 0.8 7Z", mark: "M7 4.2v3M7 8.8v.9" },
} as const;

export type IssueType = keyof typeof ISSUE_SHAPES;

/** Issue-Typ als Form **und** Farbe (UI-062). */
export function IssueTypeIcon({ type, size = 14, ...rest }: { type: IssueType; size?: number } & Omit<SVGProps<SVGSVGElement>, "type">) {
  const shape = ISSUE_SHAPES[type];
  const color = `var(--os-issue-type-${type})`;
  return (
    <svg width={size} height={size} viewBox="0 0 14 14" aria-hidden {...rest}>
      {"circle" in shape ? <circle {...shape.circle} fill={color} /> : <path d={shape.path} fill={color} />}
      <path d={shape.mark} stroke="var(--os-bg-surface)" strokeWidth="1.5" strokeLinecap="round" fill="none" />
    </svg>
  );
}
