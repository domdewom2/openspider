// Mock-Transport für vitest und den Harness (kein Kern): arbeitet mit einem gemeinsamen Zustand, damit mehrere
// Fenster (je ein Transport) wie im echten Betrieb synchron bleiben.
import type { AppSettingsDto } from "../gen/AppSettingsDto";
import type { UiEvent } from "../gen/UiEvent";
import { IpcError, localError } from "../errors";
import type { Transport } from "./types";

export type MockHandler = (args: unknown, ctx: { window: string; core: MockCore }) => unknown | Promise<unknown>;

export const DEFAULT_MOCK_SETTINGS: AppSettingsDto = {
  general: { language: "system", notifications: true },
  appearance: { theme: "system", accent: "#2f855a", zoomPct: 100, rowDensity: "compact", zebra: true, liveIntervalMs: 500 },
  storage: { crawlsDir: null, performanceProfile: "balanced", ramLimitGb: null },
  retention: { policy: "never", customDays: 30, keepLast: null },
  export: { directory: null, format: "csv" },
  network: {
    proxy: { mode: "system", kind: "http", host: "", port: 8080, username: null, bypass: [] },
    trustedCertificates: [],
  },
  accounts: { mailAccounts: [], notifyAccount: null, autoConnect: [] },
  mcp: { autostart: false },
  restartRequired: [],
} as unknown as AppSettingsDto;

/** JSON-Merge-Patch nach RFC 7396. */
export function mergePatch(target: unknown, patch: unknown): unknown {
  if (typeof patch !== "object" || patch === null || Array.isArray(patch)) return patch;
  const base: Record<string, unknown> =
    typeof target === "object" && target !== null && !Array.isArray(target) ? { ...(target as Record<string, unknown>) } : {};
  for (const [k, v] of Object.entries(patch as Record<string, unknown>)) {
    if (v === null) delete base[k];
    else base[k] = mergePatch(base[k], v);
  }
  return base;
}

function changedKeys(a: unknown, b: unknown, prefix = ""): string[] {
  if (typeof a === "object" && a !== null && typeof b === "object" && b !== null && !Array.isArray(a) && !Array.isArray(b)) {
    const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
    return [...keys].flatMap((k) =>
      changedKeys((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k], prefix ? `${prefix}.${k}` : k),
    );
  }
  return JSON.stringify(a) === JSON.stringify(b) ? [] : [prefix];
}

export interface MockCallRecord {
  window: string;
  cmd: string;
  args: unknown;
}

/** Optionale Ablage der Einstellungen (Browser-Mock: localStorage, damit mehrere Fenster synchron bleiben). */
export interface MockPersistence {
  load(): AppSettingsDto | null;
  save(dto: AppSettingsDto): void;
  /** Meldet Änderungen aus anderen Fenstern. */
  onExternalChange(cb: (dto: AppSettingsDto) => void): () => void;
}

/** Gemeinsamer Zustand aller Mock-Fenster. */
export class MockCore {
  settings: AppSettingsDto;
  uiState: { version: number; state: unknown } = { version: 1, state: {} };
  readonly calls: MockCallRecord[] = [];
  readonly handlers = new Map<string, MockHandler>();
  private readonly subs = new Map<string, Set<(e: UiEvent) => void>>();
  /** Neustartpflichtige Schlüssel (OUT-222). */
  restartKeys = new Set<string>(["storage.crawlsDir"]);

  constructor(
    settings: AppSettingsDto = DEFAULT_MOCK_SETTINGS,
    private readonly persistence?: MockPersistence,
  ) {
    this.settings = structuredClone(persistence?.load() ?? settings);
    persistence?.onExternalChange((dto) => {
      const keys = changedKeys(this.settings, dto);
      this.settings = structuredClone(dto);
      if (keys.length > 0) this.emit("*", { kind: "settingsChanged", keys });
    });
  }

  emit(window: string | "*", event: UiEvent): void {
    for (const [w, set] of this.subs) if (window === "*" || w === window) for (const l of set) l(event);
  }

  subscribers(window: string): number {
    return this.subs.get(window)?.size ?? 0;
  }

  addSub(window: string, cb: (e: UiEvent) => void): () => void {
    let set = this.subs.get(window);
    if (!set) this.subs.set(window, (set = new Set()));
    set.add(cb);
    return () => {
      set?.delete(cb);
    };
  }

  /** Transport eines Fensters. */
  transport(window = "main"): Transport {
    return {
      name: "mock",
      invoke: <T>(cmd: string, args: unknown, signal?: AbortSignal): Promise<T> => this.dispatch<T>(window, cmd, args, signal),
      subscribe: (w, onEvent) => this.addSub(w || window, onEvent),
    };
  }

  private async dispatch<T>(window: string, cmd: string, args: unknown, signal?: AbortSignal): Promise<T> {
    if (signal?.aborted) throw new IpcError(localError("Cancelled", "err-cancelled"));
    this.calls.push({ window, cmd, args });
    const custom = this.handlers.get(cmd);
    if (custom) return (await custom(args, { window, core: this })) as T;
    switch (cmd) {
      case "app.info":
        return {
          version: "0.0.0-mock",
          build: "mock",
          platform: "macos",
          arch: "aarch64",
          paths: { settings: "/mock/settings.toml", crawls: "/mock/crawls", logs: "/mock/logs", cache: "/mock/cache", exports: "/mock/exports" },
        } as unknown as T;
      case "settings.get":
        return structuredClone(this.settings) as unknown as T;
      case "settings.set": {
        const patch = (args as { patch?: unknown } | undefined)?.patch;
        const next = mergePatch(this.settings, patch) as AppSettingsDto;
        const keys = changedKeys(this.settings, next);
        this.settings = { ...next, restartRequired: keys.filter((k) => this.restartKeys.has(k)) };
        this.persistence?.save(this.settings);
        if (keys.length > 0) this.emit("*", { kind: "settingsChanged", keys });
        return structuredClone(this.settings) as unknown as T;
      }
      case "uistate.load":
        return structuredClone(this.uiState) as unknown as T;
      case "uistate.save":
        this.uiState = structuredClone(args as { version: number; state: unknown });
        return structuredClone(this.uiState) as unknown as T;
      case "events.subscribe":
        return { id: `mock-${window}` } as unknown as T;
      case "events.setVisibility":
      case "events.setInterval":
      case "diag.logFrontend":
      case "op.cancel":
        return undefined as T;
      default:
        throw new IpcError(localError("Unsupported", "err-unsupported", { command: cmd }));
    }
  }
}

export function createMockTransport(core: MockCore = new MockCore(), window = "main"): Transport & { core: MockCore } {
  return Object.assign(core.transport(window), { core });
}

/** Persistenz über `localStorage` mit `storage`-Ereignissen zwischen Fenstern desselben Ursprungs (E2E-A ohne Bridge). */
export function localStoragePersistence(key = "openspider.mock.settings"): MockPersistence {
  return {
    load() {
      try {
        const raw = localStorage.getItem(key);
        return raw ? (JSON.parse(raw) as AppSettingsDto) : null;
      } catch {
        return null;
      }
    },
    save(dto) {
      try {
        localStorage.setItem(key, JSON.stringify(dto));
      } catch {
        /* ohne Speicher bleibt der Mock flüchtig */
      }
    },
    onExternalChange(cb) {
      const handler = (e: StorageEvent) => {
        if (e.key === key && e.newValue) cb(JSON.parse(e.newValue) as AppSettingsDto);
      };
      window.addEventListener("storage", handler);
      return () => window.removeEventListener("storage", handler);
    },
  };
}
