// Entwurf der Konfiguration im Dialog (OUT-220: „Abbrechen“ verwirft ungespeicherte Änderungen): lädt `config.get`, hält Änderungen
// lokal, lässt sie von `config.validate` prüfen (Validierung ausschließlich im Kern) und schreibt erst bei „OK“ per `config.set`.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { config as configApi } from "../../../ipc/config";
import { toCmdError } from "../../../ipc/errors";
import type { CmdError } from "../../../ipc/gen/CmdError";
import type { ConfigSetResult } from "../../../ipc/gen/ConfigSetResult";
import type { CrawlConfigDto } from "../../../ipc/gen/CrawlConfigDto";
import type { DiffRow } from "../../../ipc/gen/DiffRow";
import type { ValidationIssue } from "../../../ipc/gen/ValidationIssue";
import { getPath, setPath, type ConfigJson } from "./model";
import { asSchema, fieldMeta, type ConfigSchema, type FieldMeta } from "./schema";

const VALIDATE_DELAY_MS = 200;

export interface ConfigDraft {
  /** Snapshot checked by config.validate; undefined while the first check is pending. */
  validatedConfig?: ConfigJson;
  status: "loading" | "ready" | "error";
  loadError: CmdError | null;
  /** Gespeicherter Stand (zuletzt gelesen oder geschrieben). */
  saved: ConfigJson;
  /** Bearbeiteter Stand. */
  cfg: ConfigJson;
  schema: ConfigSchema | null;
  /** Werkseinstellungs-Abweichungen des gespeicherten Standes (`config.diff` gegen die Defaults). */
  defaultRows: DiffRow[];
  issues: ValidationIssue[];
  /** Fehler des letzten `config.set` (mit Feld, wenn der Kern eines nennt). */
  saveError: CmdError | null;
  /** Felder, die erst nach einer Pause wirken (Ergebnis des letzten `config.set`). */
  deferred: string[];
  invalidatedSteps: string[];
  dirty: boolean;
  saving: boolean;
  get<T = unknown>(path: string): T;
  set(path: string, value: unknown): void;
  /** Ersetzt den ganzen Entwurf (Profil laden); der Kern prüft beim Schreiben. */
  replace(next: ConfigJson): void;
  meta(path: string): FieldMeta | null;
  /** Der Entwurf als Konfigurations-DTO (für `profile.save`). */
  toDto(): CrawlConfigDto | null;
  /** Schreibt den Entwurf. Ergebnis des Kerns bei Erfolg, sonst `null` (Fehler in `saveError`). */
  save(): Promise<ConfigSetResult | null>;
  revalidate(): void;
}

const dtoOf = (base: CrawlConfigDto, config: ConfigJson): CrawlConfigDto => ({ ...base, config: config as CrawlConfigDto["config"] });

export function useConfigDraft(active: boolean): ConfigDraft {
  const [status, setStatus] = useState<ConfigDraft["status"]>("loading");
  const [loadError, setLoadError] = useState<CmdError | null>(null);
  const [dto, setDto] = useState<CrawlConfigDto | null>(null);
  const [saved, setSaved] = useState<ConfigJson>({});
  const [cfg, setCfg] = useState<ConfigJson>({});
  const [schema, setSchema] = useState<ConfigSchema | null>(null);
  const [defaultRows, setDefaultRows] = useState<DiffRow[]>([]);
  const [validatedConfig, setValidatedConfig] = useState<ConfigJson>();
  const [issues, setIssues] = useState<ValidationIssue[]>([]);
  const [saveError, setSaveError] = useState<CmdError | null>(null);
  const [deferred, setDeferred] = useState<string[]>([]);
  const [invalidatedSteps, setInvalidated] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);
  const seq = useRef(0);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const cfgRef = useRef(cfg);
  cfgRef.current = cfg;
  const dtoRef = useRef(dto);
  dtoRef.current = dto;

  const validateNow = useCallback(async (config: ConfigJson) => {
    const base = dtoRef.current;
    if (!base) return;
    const mine = ++seq.current;
    try {
      const found = await configApi.validate(dtoOf(base, config));
      if (mine === seq.current) setValidatedConfig(config);
      if (mine === seq.current) setIssues(found);
    } catch (e) {
      if (mine === seq.current) setIssues([{ path: "", messageKey: toCmdError(e).messageKey, args: {}, warning: false, line: null, column: null }]);
    }
  }, []);

  const schedule = useCallback(
    (config: ConfigJson) => {
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => void validateNow(config), VALIDATE_DELAY_MS);
    },
    [validateNow],
  );

  useEffect(() => {
    if (!active) return;
    let live = true;
    setStatus("loading");
    setLoadError(null);
    setSaveError(null);
    setDeferred([]);
    setInvalidated([]);
    setIssues([]);
    void (async () => {
      try {
        const [got, sch, rows] = await Promise.all([
          configApi.get(),
          configApi.schema().catch(() => null),
          configApi.diff({ base: { kind: "defaults" } }).catch(() => [] as DiffRow[]),
        ]);
        if (!live) return;
        const body = (got.config ?? {}) as ConfigJson;
        setDto(got);
        dtoRef.current = got;
        setSaved(body);
        setCfg(body);
        setSchema(asSchema(sch));
        setDefaultRows(rows);
        setStatus("ready");
        void validateNow(body);
      } catch (e) {
        if (!live) return;
        setLoadError(toCmdError(e));
        setStatus("error");
      }
    })();
    return () => {
      live = false;
      if (timer.current) clearTimeout(timer.current);
    };
  }, [active, validateNow]);

  const get = useCallback(<T,>(path: string): T => getPath(cfgRef.current, path) as T, []);
  const set = useCallback(
    (path: string, value: unknown) => {
      const next = setPath(cfgRef.current, path, value);
      cfgRef.current = next;
      setCfg(next);
      setSaveError(null);
      schedule(next);
    },
    [schedule],
  );
  const replace = useCallback(
    (next: ConfigJson) => {
      cfgRef.current = next;
      setCfg(next);
      setSaveError(null);
      schedule(next);
    },
    [schedule],
  );

  const save = useCallback(async (): Promise<ConfigSetResult | null> => {
    const base = dtoRef.current;
    if (!base) return null;
    setSaving(true);
    setSaveError(null);
    try {
      const body = cfgRef.current;
      const result: ConfigSetResult = await configApi.set(dtoOf(base, body));
      setSaved(body);
      setDeferred(result.deferred);
      setInvalidated(result.invalidatedSteps);
      return result;
    } catch (e) {
      setSaveError(toCmdError(e));
      return null;
    } finally {
      setSaving(false);
    }
  }, []);

  const meta = useCallback((path: string) => fieldMeta(schema, path), [schema]);
  const dirty = useMemo(() => JSON.stringify(saved) !== JSON.stringify(cfg), [saved, cfg]);

  const allIssues = useMemo(() => {
    // Ein Chromium-Fehler ohne Feldangabe gehört zum Chromium-Pfad (Seite Rendering, CRAWL-077).
    const field = saveError?.field ?? (saveError && /chromium/i.test(`${saveError.messageKey} ${saveError.detail ?? ""}`) ? "rendering.chromium_path" : null);
    if (!saveError || !field) return issues;
    const detail: Record<string, string> = saveError.detail ? { reason: saveError.detail } : {};
    const extra: ValidationIssue = { path: field, messageKey: saveError.messageKey, args: { ...saveError.args, ...detail }, warning: false, line: null, column: null };
    return [...issues, extra];
  }, [issues, saveError]);

  return {
    validatedConfig,
    status,
    loadError,
    saved,
    cfg,
    schema,
    defaultRows,
    issues: allIssues,
    saveError,
    deferred,
    invalidatedSteps,
    dirty,
    saving,
    get,
    set,
    replace,
    meta,
    toDto: () => (dtoRef.current ? dtoOf(dtoRef.current, cfgRef.current) : null),
    save,
    revalidate: () => void validateNow(cfgRef.current),
  };
}
