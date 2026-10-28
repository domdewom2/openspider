// App-Einstellungen im Frontend (Gerüst): spiegelt `settings.toml` (OUT-222) über settings.get/settings.set, validiert mit
// zod und hält die aufgelösten Werte für Erscheinungsbild und Sprache (UI-120, UI-121).
import { create } from "zustand";
import { z } from "zod";
import { resolveLanguage } from "../i18n";
import { toCmdError, type CmdError } from "../ipc/errors";
import type { AppSettingsDto } from "../ipc/gen/AppSettingsDto";
import { settings as settingsApi } from "../ipc/settings";
import { isHexColor } from "../ui/tokens/color";
import { DEFAULT_APPEARANCE, type AppearanceState } from "../ui/tokens/theme";
import { ZOOM_STEPS } from "../ui/tokens/tokens";

export const LIVE_INTERVAL_RANGE = { min: 250, max: 5000 } as const;

const appearanceSchema = z.object({
  theme: z.enum(["light", "dark", "system"]).catch(DEFAULT_APPEARANCE.theme),
  accent: z.string().refine(isHexColor).catch(DEFAULT_APPEARANCE.accent),
  zoomPct: z
    .number()
    .refine((n) => (ZOOM_STEPS as readonly number[]).includes(n))
    .catch(DEFAULT_APPEARANCE.zoomPct),
  rowDensity: z.enum(["compact", "normal"]).catch(DEFAULT_APPEARANCE.density),
  zebra: z.boolean().catch(DEFAULT_APPEARANCE.zebra),
  liveIntervalMs: z
    .number()
    .transform((n) => Math.min(LIVE_INTERVAL_RANGE.max, Math.max(LIVE_INTERVAL_RANGE.min, Math.round(n))))
    .catch(500),
});

export interface NormalizedPrefs {
  appearance: AppearanceState;
  liveIntervalMs: number;
  /** `system` oder eine Sprachkennung. */
  languageSetting: string;
  notifications: boolean;
}

/** Macht aus einer (möglicherweise fehlerhaften) Antwort des Kerns gültige Werte; Unlesbares fällt auf Standardwerte. */
export function normalizeSettings(dto: AppSettingsDto): NormalizedPrefs {
  const a = appearanceSchema.parse(dto.appearance ?? {});
  const language = typeof dto.general?.language === "string" && dto.general.language !== "" ? dto.general.language : "system";
  return {
    appearance: { theme: a.theme, accent: a.accent, zoomPct: a.zoomPct, density: a.rowDensity, zebra: a.zebra },
    liveIntervalMs: a.liveIntervalMs,
    languageSetting: language,
    notifications: dto.general?.notifications !== false,
  };
}

type Patch = Record<string, unknown>;

export interface PrefsState {
  loaded: boolean;
  error: CmdError | null;
  dto: AppSettingsDto | null;
  appearance: AppearanceState;
  liveIntervalMs: number;
  languageSetting: string;
  /** Aufgelöste Sprache (System → Systemsprache, sonst Englisch). */
  language: string;
  notifications: boolean;
  restartRequired: string[];
  load(): Promise<void>;
  /** JSON-Merge-Patch über `AppSettingsDto` (camelCase), z. B. `{ appearance: { theme: "dark" } }`; wirkt sofort. */
  patch(patch: Patch): Promise<void>;
  /** Setzt das Erscheinungsbild auf die Standardwerte (UI-021/125). */
  resetAppearance(): Promise<void>;
}

function fromNormalized(n: NormalizedPrefs, locales?: readonly string[]): Pick<PrefsState, "appearance" | "liveIntervalMs" | "languageSetting" | "language" | "notifications"> {
  return {
    appearance: n.appearance,
    liveIntervalMs: n.liveIntervalMs,
    languageSetting: n.languageSetting,
    language: resolveLanguage(n.languageSetting, locales),
    notifications: n.notifications,
  };
}

function mergeDeep(target: unknown, patch: unknown): unknown {
  if (typeof patch !== "object" || patch === null || Array.isArray(patch)) return patch;
  const base: Record<string, unknown> = typeof target === "object" && target !== null && !Array.isArray(target) ? { ...(target as Record<string, unknown>) } : {};
  for (const [k, v] of Object.entries(patch as Record<string, unknown>)) {
    if (v === null) delete base[k];
    else base[k] = mergeDeep(base[k], v);
  }
  return base;
}

// Reads may overlap a local edit or a settingsChanged event. Only the newest
// read from the same mutation generation may replace the visible preferences.
let readGeneration = 0;
let mutationGeneration = 0;
let pendingPatches = 0;
let patchTail: Promise<unknown> = Promise.resolve();

export const usePrefs = create<PrefsState>((set, get) => ({
  loaded: false,
  error: null,
  dto: null,
  appearance: DEFAULT_APPEARANCE,
  liveIntervalMs: 500,
  languageSetting: "system",
  language: "en",
  notifications: true,
  restartRequired: [],

  async load() {
    const read = ++readGeneration;
    const mutation = mutationGeneration;
    try {
      const dto = await settingsApi.get();
      if (read !== readGeneration || mutation !== mutationGeneration || pendingPatches > 0) return;
      set({ loaded: true, error: null, dto, restartRequired: dto.restartRequired ?? [], ...fromNormalized(normalizeSettings(dto)) });
    } catch (e) {
      if (read === readGeneration && mutation === mutationGeneration && pendingPatches === 0) set({ error: toCmdError(e) });
    }
  },

  async patch(patch) {
    const mutation = ++mutationGeneration;
    ++readGeneration;
    ++pendingPatches;
    const before = get().dto;
    if (before) {
      // sofort wirken lassen (UI-120: ≤ 200 ms), bei Fehler zurücksetzen
      const optimistic = mergeDeep(before, patch) as AppSettingsDto;
      set({ dto: optimistic, ...fromNormalized(normalizeSettings(optimistic)) });
    }
    try {
      const saving = patchTail.then(() => settingsApi.set({ patch: patch as never }));
      patchTail = saving.catch(() => undefined);
      const dto = await saving;
      if (mutation === mutationGeneration) set({ error: null, dto, restartRequired: dto.restartRequired ?? [], ...fromNormalized(normalizeSettings(dto)) });
    } catch (e) {
      if (mutation === mutationGeneration) set({ error: toCmdError(e), ...(before ? { dto: before, ...fromNormalized(normalizeSettings(before)) } : {}) });
    } finally { --pendingPatches; }
  },

  async resetAppearance() {
    await get().patch({
      appearance: {
        theme: DEFAULT_APPEARANCE.theme,
        accent: DEFAULT_APPEARANCE.accent,
        zoomPct: DEFAULT_APPEARANCE.zoomPct,
        rowDensity: DEFAULT_APPEARANCE.density,
        zebra: DEFAULT_APPEARANCE.zebra,
        liveIntervalMs: 500,
      },
    });
  },
}));
