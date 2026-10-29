// i18n-Kern (UI-121, M2-ui-concept 2.7): Fluent-Bundles je Sprache aus den .ftl-Dateien der Features.
// Reine Logik ohne React und ohne Vite – die Dateien werden hineingereicht, damit sie sich testen lässt.
import { FluentBundle, FluentResource, type FluentVariable } from "@fluent/bundle";
import { exportDisplayName } from "./exportNames";
import { negotiateLanguages } from "@fluent/langneg";

export const DEFAULT_LANGUAGE = "en";

export interface FtlFile {
  /** Pfad der Datei, z. B. `/src/features/settings/frame/i18n/de.ftl`. */
  path: string;
  /** Sprache = Dateiname ohne Endung. */
  lang: string;
  /** Verzeichnis oberhalb von `i18n/`, z. B. `features/settings/frame`. */
  feature: string;
  source: string;
}

export type TArgs = Record<string, unknown>;
export interface TOptions {
  /** Englischer Kanon-Name (Tab-, Filter-, Spaltenname aus dem Kern), wenn weder die Sprache noch Englisch den Schlüssel kennen. */
  fallback?: string;
}
export type TFn = (key: string, args?: TArgs, opts?: TOptions) => string;

export function parseFtlPath(path: string): { lang: string; feature: string } | null {
  const m = /^(?:.*?\/src\/)?(.*?)\/?i18n\/([A-Za-z0-9-]+)\.ftl$/.exec(path);
  if (!m) return null;
  return { feature: m[1] ?? "", lang: m[2] as string };
}

function toVariable(v: unknown): FluentVariable {
  if (typeof v === "string" || typeof v === "number" || v instanceof Date) return v;
  if (typeof v === "boolean") return v ? "true" : "false";
  if (v === null || v === undefined) return "";
  return String(v);
}

function toVariables(args?: TArgs): Record<string, FluentVariable> | undefined {
  if (!args) return undefined;
  const out: Record<string, FluentVariable> = {};
  for (const [k, v] of Object.entries(args)) out[k] = toVariable(v);
  return out;
}

/** Aus `tab-response-codes` o. Ä. einen lesbaren Namen ableiten – letzter Notnagel, nie der rohe Schlüssel. */
export function humanizeKey(key: string): string {
  const base = key.split(".").pop() ?? key;
  const withoutPrefix = base.replace(/^(tab|filter|column|err|notice|menu|action|empty|shortcut|settings|ui)-/, "");
  const words = withoutPrefix.split(/[-_]+/).filter(Boolean);
  if (words.length === 0) return base;
  return words.map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(" ");
}

export interface I18n {
  /** Verfügbare Sprachen (Dateien), `en` immer zuerst. */
  languages(): string[];
  has(lang: string, key: string): boolean;
  /** Schlüssel in der Sprache, dann in Englisch, dann Kanon-Fallback, dann abgeleiteter Name. */
  translate(lang: string, key: string, args?: TArgs, opts?: TOptions): string;
  /** Übersetzungsfehler (Fluent), nur zur Diagnose. */
  errors(): string[];
  /** Nutzer-Locales gegen die verfügbaren Sprachen verhandeln (de-AT → de, pl → en). */
  resolve(setting: string, systemLocales: readonly string[]): string;
}

export function createI18n(files: readonly FtlFile[], onWarn: (msg: string) => void = () => undefined): I18n {
  const byLang = new Map<string, FluentBundle>();
  const problems: string[] = [];
  const bundleFor = (lang: string): FluentBundle | undefined => byLang.get(lang);

  const langs = new Set<string>([DEFAULT_LANGUAGE]);
  for (const f of files) langs.add(f.lang);
  for (const lang of langs) {
    const bundle = new FluentBundle(lang, { useIsolating: false });
    for (const f of files.filter((x) => x.lang === lang)) {
      const errs = bundle.addResource(new FluentResource(f.source), { allowOverrides: false });
      for (const e of errs) problems.push(`${f.path}: ${e.message}`);
    }
    byLang.set(lang, bundle);
  }
  const ordered = [DEFAULT_LANGUAGE, ...[...langs].filter((l) => l !== DEFAULT_LANGUAGE).sort()];

  function lookup(lang: string, key: string): { pattern: Parameters<FluentBundle["formatPattern"]>[0]; bundle: FluentBundle } | null {
    const bundle = bundleFor(lang);
    if (!bundle) return null;
    const dot = key.indexOf(".");
    const id = dot < 0 ? key : key.slice(0, dot);
    const attr = dot < 0 ? null : key.slice(dot + 1);
    const msg = bundle.getMessage(id);
    if (!msg) return null;
    const pattern = attr ? msg.attributes[attr] : msg.value;
    return pattern ? { pattern, bundle } : null;
  }

  function format(lang: string, key: string, args?: TArgs): string | null {
    const hit = lookup(lang, key);
    if (!hit) return null;
    const errs: Error[] = [];
    const text = hit.bundle.formatPattern(hit.pattern, toVariables(args), errs);
    for (const e of errs) problems.push(`${lang}/${key}: ${e.message}`);
    return text;
  }

  return {
    languages: () => [...ordered],
    has: (lang, key) => lookup(lang, key) !== null || exportDisplayName(key, (id, args) => format(lang, id, args), lang === "en" ? ":" : ": ") !== null,
    translate(lang, key, args, opts) {
      const own = format(lang, key, args) ?? exportDisplayName(key, (id, values) => format(lang, id, values), lang === "en" ? ":" : ": ");
      if (own !== null) return own;
      if (lang !== DEFAULT_LANGUAGE) {
        const en = format(DEFAULT_LANGUAGE, key, args) ?? exportDisplayName(key, (id, values) => format(DEFAULT_LANGUAGE, id, values), ":");
        if (en !== null) return en;
      }
      if (opts?.fallback) return opts.fallback;
      // Older frontends may receive a new backend error key. Keep its main text in the UI language;
      // structured callers still choose a more specific ErrorCode fallback through has().
      if (key.startsWith("err-")) {
        const error = format(lang, "err-fallback") ?? format(DEFAULT_LANGUAGE, "err-fallback");
        if (error !== null) return error;
      }
      onWarn(`i18n: Schlüssel ohne Übersetzung: ${key}`);
      return humanizeKey(key);
    },
    errors: () => [...problems],
    resolve(setting, systemLocales) {
      const available = ordered;
      if (setting !== "system" && available.includes(setting)) return setting;
      const requested = setting !== "system" ? [setting, ...systemLocales] : [...systemLocales];
      const negotiated = negotiateLanguages(requested, available, { defaultLocale: DEFAULT_LANGUAGE, strategy: "lookup" });
      return negotiated[0] ?? DEFAULT_LANGUAGE;
    },
  };
}
