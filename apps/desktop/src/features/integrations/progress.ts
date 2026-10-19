// API-Fortschritt über alle verbundenen Integrationen (INTEG-007): `Progress` = erledigte (Daten, keine Daten oder
// endgültiger Fehler) durch angeforderte URLs; `Errors` = URLs mit endgültigem Fehlerstatus. Quelle: `api.status`.
import type { ApiRow } from "../../ipc/gen/ApiRow";
import type { TFn } from "../../i18n";
import { DIALOG_KINDS } from "./sources";

export interface ApiProgressPart {
  kind: ApiRow["kind"];
  completed: number;
  requested: number;
  errors: number;
  /** GA4 lädt einen Gesamtreport: statt n/m erscheint „fertig“. */
  report: boolean;
}

export interface ApiProgress {
  /** Ohne verbundene Integration ist die Anzeige ausgeblendet. */
  visible: boolean;
  /** Ganzzahl 0 … 100, abgerundet: 100 % erst, wenn alles erledigt ist. */
  percent: number;
  parts: ApiProgressPart[];
}

const order = (kind: string) => {
  const i = (DIALOG_KINDS as readonly string[]).indexOf(kind);
  return i < 0 ? DIALOG_KINDS.length : i;
};

export function apiProgress(rows: readonly ApiRow[]): ApiProgress {
  const connected = rows.filter((r) => r.connected).sort((a, b) => order(a.kind) - order(b.kind));
  let done = 0;
  let total = 0;
  const parts = connected.map((r) => {
    const completed = Math.min(r.completed, r.requested);
    done += completed;
    total += r.requested;
    return { kind: r.kind, completed, requested: r.requested, errors: r.errors, report: r.kind === "ga4" };
  });
  return { visible: connected.length > 0, percent: total > 0 ? Math.floor((done * 100) / total) : 0, parts };
}

/** Aufschlüsselung je Integration für den Tooltip, z. B. `PSI 120/400 (3 Fehler), GA4 fertig (0 Fehler)`. */
export function progressTooltip(t: TFn, progress: ApiProgress): string {
  return progress.parts
    .map((p) => {
      const name = t(`integ-short-${p.kind}`);
      const finished = p.report && p.requested > 0 && p.completed >= p.requested;
      const amount = finished ? t("integ-progress-finished") : `${p.completed}/${p.requested}`;
      return t("integ-progress-part", { name, amount, errors: p.errors });
    })
    .join(", ");
}
