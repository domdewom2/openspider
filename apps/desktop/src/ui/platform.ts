// Plattform für Kürzel, Dialogfußleisten und Schriftstapel (M2-ui-concept 5.6). Linux (Cloud-Tests) verhält sich wie Windows.
import type { Platform } from "./tokens/tokens";

export type { Platform };

let override: Platform | null = null;

/** Nur für Tests und die Harness-Seite (`?platform=mac`). */
export function setPlatformOverride(p: Platform | null): void {
  override = p;
}

export function detectPlatform(nav: { platform?: string; userAgent?: string; userAgentData?: { platform?: string } } = navigator): Platform {
  if (override) return override;
  const source = `${nav.userAgentData?.platform ?? ""} ${nav.platform ?? ""} ${nav.userAgent ?? ""}`.toLowerCase();
  return /mac|iphone|ipad/.test(source) ? "mac" : "win";
}

export function currentPlatform(): Platform {
  return typeof navigator === "undefined" ? "win" : detectPlatform(navigator);
}
