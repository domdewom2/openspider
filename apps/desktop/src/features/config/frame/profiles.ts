// Profile laden (OUT-220, CRAWL-126): Während „Läuft“ gesperrt, im pausierten Zustand nur für Optionen, die sich zur Laufzeit ändern
// lassen. Das geladene Profil ist zunächst nur ein Entwurf des Dialogs; angewendet wird es erst mit „OK“ über `config.set`.
import { useCallback, useEffect, useState } from "react";
import { profile as profileApi } from "../../../ipc/profile";
import { toCmdError } from "../../../ipc/errors";
import type { CmdError } from "../../../ipc/gen/CmdError";
import type { ConfigProfileInfo } from "../../../ipc/gen/ConfigProfileInfo";
import { changedPaths, lockReason, type ConfigJson, type RunMode } from "./model";

export type LoadCheck =
  | { ok: true }
  | { ok: false; reason: "running" | "foreign" }
  /** Das Profil ändert Felder, die in diesem Zustand nicht änderbar sind (neuer Crawl nötig bzw. Pause nötig). */
  | { ok: false; reason: "locked"; fields: string[] };

const META = new Set(["generator", "schema_version"]);

/** Darf das Profil in den Entwurf übernommen werden? */
export function checkProfileLoad(mode: RunMode, current: ConfigJson, loaded: ConfigJson): LoadCheck {
  if (mode === "running") return { ok: false, reason: "running" };
  if (mode === "foreign") return { ok: false, reason: "foreign" };
  if (mode === "free") return { ok: true };
  const fields = changedPaths(current, loaded)
    .filter((p) => !META.has(p))
    .filter((p) => lockReason(p, mode) !== null);
  return fields.length > 0 ? { ok: false, reason: "locked", fields } : { ok: true };
}

export interface ProfilesState {
  profiles: ConfigProfileInfo[];
  loading: boolean;
  error: CmdError | null;
  reload(): Promise<void>;
}

/** Liste der Profile (`profile.list`); `active` steuert, wann geladen wird. */
export function useProfileList(active: boolean): ProfilesState {
  const [profiles, setProfiles] = useState<ConfigProfileInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<CmdError | null>(null);
  const reload = useCallback(async () => {
    setLoading(true);
    try {
      setProfiles(await profileApi.list());
      setError(null);
    } catch (e) {
      setError(toCmdError(e));
    } finally {
      setLoading(false);
    }
  }, []);
  useEffect(() => {
    if (active) void reload();
  }, [active, reload]);
  return { profiles, loading, error, reload };
}
