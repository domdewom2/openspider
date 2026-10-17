import type { ReactNode } from "react";
import { ProgressBar as RacProgressBar } from "react-aria-components";
import { AlertTriangle, CircleAlert, Info, X } from "lucide-react";
import { cx } from "./cx";
import { IconButton } from "./Button";

export type BannerKind = "info" | "warning" | "error";

const ICONS = { info: Info, warning: AlertTriangle, error: CircleAlert } as const;

/** Nicht modale Meldung (Banner/Toast) mit optionaler Aktion; Form + Text, nie nur Farbe. */
export function Banner({ kind = "info", children, action, onDismiss, dismissLabel }: {
  kind?: BannerKind;
  children: ReactNode;
  action?: ReactNode;
  onDismiss?: () => void;
  dismissLabel?: string;
}) {
  const Icon = ICONS[kind];
  return (
    <div className={cx("os-banner", `os-banner-${kind}`)} role={kind === "error" ? "alert" : "status"}>
      <Icon size={14} aria-hidden />
      <div className="os-banner-text">{children}</div>
      {action}
      {onDismiss && <IconButton label={dismissLabel ?? "×"} icon={<X size={12} />} onPress={onDismiss} />}
    </div>
  );
}

/** Bereich für Toasts; `aria-live="polite"` nur für seltene Zustandswechsel (2.10). */
export function ToastRegion({ children, label }: { children?: ReactNode; label: string }) {
  return (
    <div className="os-toast-region" role="region" aria-label={label} aria-live="polite">
      {children}
    </div>
  );
}

/** Fortschrittsbalken (determiniert, wenn `value` gesetzt ist). */
export function ProgressBar({ label, value, max = 100 }: { label: string; value?: number | null; max?: number }) {
  const determinate = value !== undefined && value !== null;
  return (
    <RacProgressBar className="os-progress" aria-label={label} value={determinate ? value : undefined} maxValue={max} isIndeterminate={!determinate}>
      {({ percentage }) => <div className="os-progress-track"><div className="os-progress-fill" style={determinate ? { width: `${percentage ?? 0}%` } : undefined} /></div>}
    </RacProgressBar>
  );
}

/** Kleines Etikett (Segment, Issue-Typ, Priorität). */
export function Badge({ children, tone = "neutral", color }: { children: ReactNode; tone?: "neutral" | "accent" | "warning" | "danger"; color?: string }) {
  return (
    <span className={cx("os-badge", `os-badge-${tone}`)} style={color ? { background: color } : undefined}>
      {children}
    </span>
  );
}
