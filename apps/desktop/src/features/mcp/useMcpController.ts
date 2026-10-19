import { useCallback, useEffect, useRef, useState } from "react";
import { eventBus } from "../../ipc/events";
import { mcp } from "../../ipc/mcp";
import { settings } from "../../ipc/settings";
import type { McpSettingsDto } from "../../ipc/gen/McpSettingsDto";
import type { Notice } from "../../ipc/gen/Notice";
import { useSession } from "../../state/session";
import { errorKey, useMcpUi } from "./model";

/** One host consumes the existing window event bus; no second transport or status polling. */
export function useMcpController() {
  const [prefs, setPrefs] = useState<McpSettingsDto | null>(null);
  const [available, setAvailable] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const locked = useRef(false);
  const alive = useRef(false);
  const statusVersion = useRef(0);
  const settingsVersion = useRef(0);
  const report = useCallback((e: unknown) => { if (alive.current) setError(errorKey(e)); }, []);
  const refreshSettings = useCallback(async () => {
    const version = ++settingsVersion.current;
    const dto = await settings.get();
    if (alive.current && version === settingsVersion.current) setPrefs(dto.mcp);
  }, []);
  const refresh = useCallback(async () => {
    const version = ++statusVersion.current;
    const results = await Promise.allSettled([mcp.status(), refreshSettings()]);
    if (!alive.current) return;
    const status = results[0];
    if (version === statusVersion.current) {
      if (status.status === "fulfilled") {
        useMcpUi.setState({ status: status.value });
        setAvailable(true);
      } else { setAvailable(false); report(status.reason); }
    }
    if (results[1].status === "rejected") report(results[1].reason);
  }, [refreshSettings, report]);
  useEffect(() => {
    alive.current = true;
    const current = useSession.getState().mcp;
    if (current) useMcpUi.setState({ status: current });
    const notice = (value: Notice) => {
      if (value.level === "error" && /^mcp(?:[._-]|$)/i.test(value.code)) setError("mcp-command-failed");
    };
    useSession.getState().notices.forEach(notice);
    let lastSeq = useSession.getState().seq;
    const offTick = eventBus.on("tick", tick => {
      if (tick.seq <= lastSeq) return;
      lastSeq = tick.seq;
      tick.notices.forEach(notice);
      if (tick.mcp) {
        statusVersion.current++;
        useMcpUi.setState({ status: tick.mcp });
        setAvailable(true);
      }
    });
    const offSettings = eventBus.on("settingsChanged", keys => {
      if (keys.some(key => key === "mcp" || key.startsWith("mcp."))) {
        void refreshSettings().then(() => { if (alive.current) setRevision(v => v + 1); }).catch(report);
      }
    });
    const offNotice = eventBus.on("notice", notice);
    void refresh();
    return () => { alive.current = false; statusVersion.current++; settingsVersion.current++; offTick(); offSettings(); offNotice(); };
  }, [refresh, refreshSettings, report]);

  const perform = async (action: () => Promise<void>) => {
    if (locked.current) return;
    locked.current = true;
    setBusy(true);
    setError(null);
    try { await action(); }
    catch (e) { report(e); }
    finally {
      locked.current = false;
      if (alive.current) { setBusy(false); setRevision(v => v + 1); }
    }
  };
  const changeServer = (port: number | null, stop = false) => perform(async () => {
    const version = ++statusVersion.current;
    const status = stop ? await mcp.stop() : await mcp.start({ port });
    if (alive.current && version === statusVersion.current) useMcpUi.setState({ status });
    await refreshSettings();
  });
  const autostart = (value: boolean) => perform(async () => {
    await mcp.setAutostart({ value });
    await refreshSettings();
  });
  return { prefs, available: available && prefs !== null, busy, error, revision, report, setError, refresh, refreshSettings, perform, changeServer, autostart };
}
export type McpController = ReturnType<typeof useMcpController>;
