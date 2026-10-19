// Fortschrittsanzeige „API: 73 %“ neben dem Crawl-Fortschritt (INTEG-007); Tooltip mit Aufschlüsselung je
// Integration und Fehlerzahl. Ohne verbundene Integration ausgeblendet. Aktualisierung über die Tick-Versionen.
import { useEffect, useRef, useState } from "react";
import { useT } from "../../i18n";
import { api } from "../../ipc/api";
import type { ApiRow } from "../../ipc/gen/ApiRow";
import { useSession } from "../../state/session";
import { Button, ProgressBar, Tooltip } from "../../ui/primitives";
import { apiProgress, progressTooltip } from "./progress";
import { openApiAccess, useIntegrations } from "./store";

const STATUS_INTERVAL_MS = 500;

/** Version, die sich ändert, sobald eine API-relevante Operation ihren Zustand wechselt. */
function useOpsVersion(): string {
  return useSession((s) =>
    Object.values(s.ops)
      .filter((o) => o.labelKey.startsWith("op-api-"))
      .map((o) => `${o.opId}:${o.state}:${o.done}`)
      .join("|"),
  );
}

export function ApiProgress() {
  const t = useT();
  const [rows, setRows] = useState<ApiRow[]>([]);
  const sidebarVersion = useSession((s) => s.sidebarVersion);
  const countsVersion = useSession((s) => s.countsVersion);
  const crawlId = useSession((s) => s.crawl?.crawlId ?? null);
  const opsVersion = useOpsVersion();
  // Der Dialog verbindet und trennt; nach dem Schließen den Stand neu holen.
  const dialogOpen = useIntegrations((s) => s.dialog !== null);
  const last = useRef(0);
  useEffect(() => {
    let live = true;
    // Ticks kommen bis zu zehnmal je Sekunde; `api.status` höchstens alle STATUS_INTERVAL_MS, der letzte Stand kommt immer an.
    const wait = Math.max(0, last.current + STATUS_INTERVAL_MS - Date.now());
    const timer = setTimeout(() => {
      last.current = Date.now();
      api.status().then(
        (r) => live && setRows(r),
        () => undefined,
      );
    }, wait);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [sidebarVersion, countsVersion, crawlId, opsVersion, dialogOpen]);
  const progress = apiProgress(rows);
  if (!progress.visible) return null;
  const label = t("integ-progress-label", { percent: progress.percent });
  return (
    <div className="os-integ-progress" data-testid="api-progress">
      <Tooltip text={progressTooltip(t, progress)}>
        <Button variant="tool" aria-label={`${label}. ${progressTooltip(t, progress)}`} onPress={() => openApiAccess()}>
          {label}
        </Button>
      </Tooltip>
      <ProgressBar label={t("integ-progress-name")} value={progress.percent} max={100} />
    </div>
  );
}
