// Reine Logik des Konfigurationsdialogs (M2-U13): Pfadzugriff auf die Konfiguration, Sperren je Crawl-Zustand
// (CRAWL-126, F-13/F-14), Abweichungen gegen die Werkseinstellungen (OUT-224) und Zuordnung von Sprungzielen.
import type { CrawlStatus } from "../../../ipc/gen/CrawlStatus";
import type { DiffRow } from "../../../ipc/gen/DiffRow";
import type { ValidationIssue } from "../../../ipc/gen/ValidationIssue";

export type ConfigJson = Record<string, unknown>;

/** Wert unter einem Punktpfad (`speed.threads`); `undefined`, wenn der Pfad fehlt. */
export function getPath(obj: unknown, path: string): unknown {
  let cur: unknown = obj;
  for (const seg of path.split(".")) {
    if (typeof cur !== "object" || cur === null || Array.isArray(cur)) return undefined;
    cur = (cur as ConfigJson)[seg];
  }
  return cur;
}

/** Kopie von `obj` mit neuem Wert unter dem Punktpfad; fehlende Zwischenobjekte werden angelegt, `obj` bleibt unverändert. */
export function setPath(obj: ConfigJson, path: string, value: unknown): ConfigJson {
  const [head, ...rest] = path.split(".");
  if (head === undefined) return obj;
  if (rest.length === 0) return { ...obj, [head]: value };
  const child = obj[head];
  const base = typeof child === "object" && child !== null && !Array.isArray(child) ? (child as ConfigJson) : {};
  return { ...obj, [head]: setPath(base, rest.join("."), value) };
}

// --- Sperren nach Crawl-Zustand ---------------------------------------------------------------------------------------

/** Zustand des Dialogs gegenüber dem Crawl: frei, läuft, pausiert/abgeschlossen (Sitzung vorhanden) oder fremd gestartet. */
export type RunMode = "free" | "running" | "paused" | "foreign";

export type LockReason = "pause" | "newCrawl" | "foreign";

/**
 * Felder, die zur Laufzeit wirken (F-13, Abbild von `live_field` in `os-app`): Schwellwerte/Voreinstellungen, Duplikate, Links,
 * Segmente, Crawl-Analyse, `speed.threads` und `issues.ignore_non_indexable`.
 */
export const PRESET_KEYS: readonly string[] = [
  "title.max_chars",
  "title.min_chars",
  "title.max_px",
  "title.min_px",
  "meta_description.max_chars",
  "meta_description.min_chars",
  "meta_description.max_px",
  "meta_description.min_px",
  "url.max_chars",
  "h1.max_chars",
  "h2.max_chars",
  "image.max_kb",
  "image.alt_max_chars",
  "content.low_word_count",
  "content.soft404_phrases",
  "links.high_crawl_depth",
  "links.high_internal_outlinks",
  "links.high_external_outlinks",
  "links.non_descriptive_anchors",
  "validation.html_doc_max_bytes",
  "validation.resource_max_bytes",
];

export function isLiveField(path: string): boolean {
  return (
    PRESET_KEYS.includes(path) ||
    ["duplicates.", "links.", "segments.", "analysis."].some((p) => path.startsWith(p)) ||
    path === "speed.threads" ||
    path === "issues.ignore_non_indexable"
  );
}

/** Auch nach einer Pause nicht änderbar (CRAWL-126: „neuer Crawl nötig“); Abbild von `check_state` in `os-app`. */
export const STRUCTURAL_FIELDS: readonly string[] = ["mode", "rendering.mode", "scope.all_subdomains", "scope.subfolder_only", "scope.crawl_outside_start_folder"];

export function runModeOf(crawl: Pick<CrawlStatus, "phase" | "owner"> | null | undefined): RunMode {
  if (!crawl || crawl.phase === "idle") return "free";
  if (!crawl.owner.controllable) return "foreign";
  return crawl.phase === "running" ? "running" : "paused";
}

/** Warum ein Feld gesperrt ist; `null` = editierbar. */
export function lockReason(path: string, mode: RunMode): LockReason | null {
  if (mode === "free") return null;
  if (mode === "foreign") return "foreign";
  if (STRUCTURAL_FIELDS.includes(path)) return "newCrawl";
  if (mode === "running" && !isLiveField(path)) return "pause";
  return null;
}

/** Pfade, die sich zwischen zwei Konfigurationen unterscheiden (Objekte rekursiv, Listen und Werte als Blatt). */
export function changedPaths(a: unknown, b: unknown, prefix = ""): string[] {
  if (JSON.stringify(a) === JSON.stringify(b)) return [];
  if (isObject(a) && isObject(b)) {
    const keys = [...new Set([...Object.keys(a), ...Object.keys(b)])].sort();
    return keys.flatMap((k) => changedPaths(a[k], b[k], prefix ? `${prefix}.${k}` : k));
  }
  return [prefix];
}

function isObject(v: unknown): v is ConfigJson {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

// --- Abweichungen von den Werkseinstellungen (OUT-224) -----------------------------------------------------------------

const META_PATHS = new Set(["generator", "schema_version"]);

/**
 * Werkseinstellung je Pfad aus `config.diff(defaults)`: Das Ergebnis nennt nur Pfade, die in der gespeicherten Konfiguration
 * abweichen; für alle anderen gilt der gespeicherte Wert als Standard.
 */
export function defaultResolver(saved: unknown, rows: readonly DiffRow[]): (path: string) => unknown {
  const fromRows = new Map(rows.map((r) => [r.path, r.base]));
  return (path) => (fromRows.has(path) ? fromRows.get(path) : getPath(saved, path));
}

export interface Deviation {
  path: string;
  value: unknown;
  standard: unknown;
}

/** Abweichungen des Entwurfs von den Werkseinstellungen, stabil nach Pfad sortiert. */
export function deviations(draft: unknown, saved: unknown, rows: readonly DiffRow[]): Deviation[] {
  const standard = defaultResolver(saved, rows);
  const paths = new Set<string>();
  const collect = (v: unknown, prefix: string) => {
    if (isObject(v)) for (const [k, x] of Object.entries(v)) collect(x, prefix ? `${prefix}.${k}` : k);
    else if (prefix) paths.add(prefix);
  };
  collect(draft, "");
  for (const r of rows) paths.add(r.path);
  const out: Deviation[] = [];
  for (const path of [...paths].sort()) {
    if (META_PATHS.has(path)) continue;
    const value = getPath(draft, path) ?? null;
    const base = standard(path) ?? null;
    if (JSON.stringify(value) !== JSON.stringify(base)) out.push({ path, value, standard: base });
  }
  return out;
}

// --- Prüfergebnisse je Feld ---------------------------------------------------------------------------------------------

/** Meldungen zu einem Feld: gleicher Pfad oder ein Unterpfad (`scope.include[2]` gehört zu `scope.include`). */
export function issuesFor(issues: readonly ValidationIssue[], path: string): ValidationIssue[] {
  return issues.filter((i) => i.path === path || i.path.startsWith(`${path}[`) || i.path.startsWith(`${path}.`));
}

/** Index einer Listenzeile aus `scope.include[2]` (0-basiert); `null` ohne Index. */
export function lineIndexOf(path: string): number | null {
  const m = /\[(\d+)\]/.exec(path);
  return m ? Number(m[1]) : null;
}

// --- Sprungziele --------------------------------------------------------------------------------------------------------

export interface AnchorPage {
  id: string;
  anchors: readonly string[];
}

/**
 * Seite zu einem Sprungziel (`openConfig({ anchor })`): genaue Übereinstimmung mit einem Feldpfad der Seite hat Vorrang vor
 * dem längsten passenden Präfix (`rendering` → Rendering, `speed.threads` → Geschwindigkeit).
 */
export function pageForAnchor(pages: readonly AnchorPage[], anchor: string): AnchorPage | null {
  const exact = pages.find((p) => p.anchors.includes(anchor));
  if (exact) return exact;
  let best: { page: AnchorPage; len: number } | null = null;
  for (const page of pages) {
    for (const a of page.anchors) {
      if (anchor.startsWith(`${a}.`) || anchor.startsWith(`${a}[`) || a.startsWith(`${anchor}.`)) {
        const len = a.length;
        if (!best || len > best.len) best = { page, len };
      }
    }
  }
  return best?.page ?? null;
}
