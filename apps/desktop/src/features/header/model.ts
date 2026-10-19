import { create } from "zustand";
import type { CrawlModeDto } from "../../ipc/gen/CrawlModeDto";
import type { StartScope } from "../../ipc/gen/StartScope";

/** UI-003 deliberately defaults to HTTPS; CRAWL-001's HTTP default conflicts (reported by U03). */
export function normalizeInput(input: string): string | null {
  const text = input.trim();
  if (!text || /[\r\n\t]/.test(text) || /^(?!https?:)[a-z][a-z\d+.-]*:/i.test(text) && !/^[^/:\s]+:\d+(?:\/|$)/.test(text)) return null;
  // Browser URL parsers differ in accepting spaces in authorities; enforce the UI contract first.
  const authority = text.replace(/^https?:\/\//i, "").split(/[/?#]/)[0] ?? "";
  if (/\s|%(?:20|09|0a|0d)/i.test(authority)) return null;
  try {
    const url = new URL(/^https?:\/\//i.test(text) ? text : `https://${text}`);
    if (!url.hostname || url.username || url.password || !["https:", "http:"].includes(url.protocol)) return null;
    return url.href;
  } catch {
    return null;
  }
}

export function missingFolderSlash(url: string | null, scope: StartScope): boolean {
  if (!url || scope !== "folder") return false;
  const path = new URL(url).pathname;
  return !path.endsWith("/") && !path.slice(path.lastIndexOf("/") + 1).includes(".");
}

export function rememberUrl(history: readonly string[], url: string): string[] {
  return [url, ...history.filter((u) => u !== url)].slice(0, 20);
}

/** Header choices are view state; the crawl itself is only mirrored from ticks. */
export const useHeader = create<{
  mode: CrawlModeDto;
  url: string;
  scope: StartScope;
}>(() => ({ mode: "spider", url: "", scope: "domain" }));
