// Öffentliche i18n-Schnittstelle: `useT()`, `setLanguage()`, Formatierer (UI-121).
// Ressourcen: src/**/i18n/<sprache>.ftl, per import.meta.glob gebündelt (je Feature, keine gemeinsame Datei).
import { useMemo } from "react";
import { create } from "zustand";
import { createI18n, parseFtlPath, type FtlFile, type I18n, type TFn } from "./core";
import { makeFormatters, type Formatters } from "./format";

export type { FtlFile, I18n, TFn, TArgs, TOptions } from "./core";
export { humanizeKey, createI18n, parseFtlPath, DEFAULT_LANGUAGE } from "./core";
export { makeFormatters, type Formatters } from "./format";

const raw = import.meta.glob<string>("/src/**/i18n/*.ftl", { query: "?raw", import: "default", eager: true });

export function loadFtlFiles(): FtlFile[] {
  const files: FtlFile[] = [];
  for (const [path, source] of Object.entries(raw)) {
    const parsed = parseFtlPath(path);
    if (!parsed) continue;
    if (parsed.lang === "n03") {
      // N03 owns one bilingual sidecar per feature, isolated from the catalog work.
      for (const section of source.split(/^# @language /m).slice(1)) {
        const newline = section.indexOf("\n");
        const lang = section.slice(0, newline).trim();
        if (lang === "de" || lang === "en") files.push({ path, lang, feature: parsed.feature, source: section.slice(newline + 1) });
      }
    } else files.push({ path, lang: parsed.lang, feature: parsed.feature, source });
  }
  return files;
}

const warned = new Set<string>();
export const i18n: I18n = createI18n(loadFtlFiles(), (msg) => {
  if (import.meta.env.DEV && !warned.has(msg)) {
    warned.add(msg);
    console.warn(msg);
  }
});

export type Lang = string;

interface LanguageState {
  lang: Lang;
}
export const useLanguageStore = create<LanguageState>(() => ({ lang: "en" }));

export function currentLanguage(): Lang {
  return useLanguageStore.getState().lang;
}

type LanguageListener = (lang: Lang) => void | Promise<void>;
const listeners = new Set<LanguageListener>();

/** Registriert einen Beobachter, der beim Sprachwechsel läuft (z. B. der Menüneuaufbau). */
export function onLanguageChanged(cb: LanguageListener): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

/** Systemsprachen des Nutzers (`navigator.languages`), UI-121: de-AT/de-CH → de, sonst Englisch. */
export function systemLocales(nav: Pick<Navigator, "languages" | "language"> = navigator): string[] {
  const list = nav.languages?.length ? [...nav.languages] : nav.language ? [nav.language] : [];
  return list.length > 0 ? list : ["en"];
}

/** Löst die Einstellung (`system` | `de` | `en` | …) in eine verfügbare Sprache auf. */
export function resolveLanguage(setting: string, locales: readonly string[] = systemLocales()): Lang {
  return i18n.resolve(setting, locales);
}

/**
 * Wechselt die Sprache ohne Neustart: Bundles sind da, der Store löst das Neuzeichnen aus, danach laufen alle
 * Beobachter (Menüneuaufbau). Stores und Zustand bleiben unberührt.
 */
export async function setLanguage(lang: Lang): Promise<void> {
  const next = i18n.languages().includes(lang) ? lang : "en";
  useLanguageStore.setState({ lang: next });
  if (typeof document !== "undefined") document.documentElement.lang = next;
  await Promise.all([...listeners].map((l) => l(next)));
}

const tCache = new Map<Lang, TFn>();
export function makeT(lang: Lang): TFn {
  let t = tCache.get(lang);
  if (!t) {
    const resolved = lang;
    t = (key, args, opts) => i18n.translate(resolved, key, args, opts);
    tCache.set(lang, t);
  }
  return t;
}

/** Übersetzer für Komponenten; rendert bei Sprachwechsel neu. */
export function useT(): TFn {
  const lang = useLanguageStore((s) => s.lang);
  return useMemo(() => makeT(lang), [lang]);
}

/** Übersetzer außerhalb von React (Menü, Fehlertexte) in der aktuellen Sprache. */
export function getT(): TFn {
  return makeT(currentLanguage());
}

const fCache = new Map<Lang, Formatters>();
export function getFormatters(lang: Lang = currentLanguage()): Formatters {
  let f = fCache.get(lang);
  if (!f) fCache.set(lang, (f = makeFormatters(lang)));
  return f;
}

export function useFormat(): Formatters {
  const lang = useLanguageStore((s) => s.lang);
  return useMemo(() => getFormatters(lang), [lang]);
}
