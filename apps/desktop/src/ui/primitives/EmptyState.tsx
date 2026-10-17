import { CircleSlash, FileSearch, Hourglass, Info, KeyRound, Lock, MousePointerClick, Settings2 } from "lucide-react";
import type { ReactNode } from "react";
import { useT } from "../../i18n";
import { emptyMessageKey, emptyStateAttr, type EmptyAction, type EmptyState as EmptyStateDto } from "../emptyState/codes";
import { Button } from "./Button";

const ICONS: Record<EmptyStateDto["code"], typeof Info> = {
  NO_SELECTION: MousePointerClick,
  MODE_UNSUPPORTED: CircleSlash,
  NEEDS_JS_RENDERING: Settings2,
  FEATURE_DISABLED: Lock,
  HTML_NOT_STORED: FileSearch,
  RENDERED_HTML_NOT_STORED: FileSearch,
  API_NOT_CONNECTED: KeyRound,
  NEEDS_CRAWL_ANALYSIS: Hourglass,
  NO_DATA_YET: Hourglass,
  NO_MATCH: FileSearch,
  NOT_APPLICABLE: Info,
};

export function actionLabelKey(a: EmptyAction): string {
  return "empty-action-" + a.kind.replace(/([A-Z])/g, "-$1").toLowerCase();
}

/**
 * Hinweisfläche für leere Tabellen und fehlende Voraussetzungen (UI-016): Symbol, Text mit konkretem Namen der
 * Einstellung bzw. des Teilschritts, Aktionsschaltfläche; maschinenlesbar als `data-empty-state="<CODE>"`.
 */
export function EmptyState({ state, onAction, children }: { state: EmptyStateDto; onAction?: (a: EmptyAction) => void; children?: ReactNode }) {
  const t = useT();
  const Icon = ICONS[state.code];
  const arg = state.arg ?? "";
  return (
    <div className="os-empty" role="status" data-empty-state={emptyStateAttr(state)}>
      <Icon size={20} aria-hidden className="os-empty-icon" />
      <p className="os-empty-text">{t(emptyMessageKey(state.code), { arg })}</p>
      {state.action && onAction && (
        <Button variant="secondary" onPress={() => onAction(state.action as EmptyAction)}>
          {t(actionLabelKey(state.action))}
        </Button>
      )}
      {children}
    </div>
  );
}
