// UI-121: export catalog names are stable command arguments, and may contain spaces/colons.
// Resolve their display names here so menus and export dialogs share the same translations.
import { canonKey } from "./canon";

type Lookup = (key: string, args?: Record<string, unknown>) => string | null;

export function exportNameKey(name: string): string {
  return `export-name-${name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "")}`;
}

export function exportDisplayName(name: string, lookup: Lookup, separator = ": "): string | null {
  // The catalog owns export kind and name provenance. Never infer them from user text.
  if (name.startsWith("custom-export:")) {
    let metadata: unknown;
    try { metadata = JSON.parse(name.slice("custom-export:".length)); } catch { return null; }
    if (!metadata || typeof metadata !== "object") return null;
    const data = metadata as Record<string, unknown>;
    if ((data.group !== "search" && data.group !== "extraction") || typeof data.name !== "string" || typeof data.inlinks !== "boolean") return null;
    const group = lookup(canonKey("tab", data.group === "search" ? "Custom Search" : "Custom Extraction"));
    if (group === null) return null;
    let label = data.name;
    if (typeof data.generatedIndex === "number") {
      const key = data.group === "search" ? "export-name-filter" : "export-name-extractor";
      const generated = lookup(key, { n: data.generatedIndex });
      if (generated === null) return null;
      label = generated;
    }
    if (data.inlinks) {
      const inlinks = lookup("export-name-inlinks", { name: label });
      if (inlinks === null) return null;
      label = inlinks;
    }
    return `${group}${separator}${label}`;
  }
  if (name.startsWith("report '")) {
    const clauses = [...name.matchAll(/(?:^|; )report '([^']+)' requires ([\s\S]*?)(?=; report '|$)/g)];
    if (clauses.length) {
      const messages = clauses.map(([, report, requirement]) => {
        const label = exportDisplayName(report!, lookup, separator) ?? report!;
        const kind = requirement!.startsWith("the crawl analysis:") ? "analysis" : ({
          SitemapProcessed: "sitemap", OrphanSourceAvailable: "orphans",
          "JavaScript rendering": "rendering", "cookie extraction": "cookies",
          "HTTP header extraction": "headers", "HTML storage": "html",
          "rendered HTML storage": "rendered-html", "stored rendered HTML": "rendered-html",
          "saved screenshots": "screenshots", "spell checking": "spelling",
          "grammar checking": "grammar", "accessibility extraction": "a11y",
          "HTML validation": "validation", "JavaScript error reporting": "javascript-errors",
        } as Record<string, string>)[requirement!] ?? "missing";
        return lookup(`export-reason-${kind}`, { name: label });
      });
      return messages.every((value): value is string => value !== null) ? messages.join(" ") : null;
    }
  }
  // Remaining keys are catalog-owned labels. User names arrive exclusively through
  // custom-export metadata above, even when their text matches a static label.
  const part = (value: string): string | null => {
    const direct = lookup(exportNameKey(value)) ?? lookup(canonKey("tab", value)) ?? lookup(canonKey("filter", value));
    if (direct !== null) return direct;
    for (const [pattern, key] of [
      [/^Images Over (\d+) KB$/, "export-name-images-over-kb"],
      [/^Images With Alt Text Over (\d+) Characters$/, "export-name-images-alt-over-characters"],
    ] as const) {
      const match = pattern.exec(value);
      if (match) return lookup(key, { n: match[1] });
    }
    for (const [suffix, key] of [[" (Bulk)", "export-name-bulk"], [" Inlinks", "export-name-inlinks"], [" Details", "export-name-details"], [" Triples", "export-name-triples"]] as const) {
      if (value.endsWith(suffix)) {
        const translated = part(value.slice(0, -suffix.length));
        if (translated !== null) return lookup(key, { name: translated });
      }
    }
    return null;
  };
  const parts = name.split(":").map(value => part(value.trim()));
  return parts.every((value): value is string => value !== null) ? parts.join(separator) : null;
}
