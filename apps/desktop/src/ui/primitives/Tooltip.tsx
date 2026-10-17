import type { ReactNode } from "react";
import { Tooltip as RacTooltip, TooltipTrigger } from "react-aria-components";

/** Tooltip nach 300 ms (5.4); der Auslöser ist ein fokussierbares Element. */
export function Tooltip({ text, children }: { text: string; children: ReactNode }) {
  return (
    <TooltipTrigger delay={300}>
      {children}
      <RacTooltip className="os-tooltip">{text}</RacTooltip>
    </TooltipTrigger>
  );
}
