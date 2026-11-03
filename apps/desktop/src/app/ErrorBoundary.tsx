import { Component, type ErrorInfo, type ReactNode } from "react";
import { create } from "zustand";
import { crash } from "../ipc/crash";
import { diag } from "../ipc/diag";
import { getT } from "../i18n";
import { Button } from "../ui/primitives/Button";

/**
 * Meldet einen Fehler, der eine Fehlergrenze ausgelöst hat, als lokalen Absturzbericht `frontend` (OUT-246). Der Kern
 * begrenzt auf 5 je Lauf und schreibt gleiche Meldung und Stelle nur einmal; gesendet wird hier nichts (OUT-247).
 */
export function reportBoundaryError(region: string, error: unknown, info: ErrorInfo): void {
  const message = error instanceof Error ? error.message : String(error);
  if (import.meta.env.DEV && message.startsWith(PROBE_MESSAGE)) useCrashProbe.setState({ region: null });
  void crash.reportFrontend({ message, componentStack: info.componentStack ?? "", region }).catch(() => undefined);
}

/** Testbefehl für einen Fehler in einer Fehlergrenze (OUT-246), nur in Entwicklungs- und Test-Builds. */
export const useCrashProbe = create<{ region: string | null }>(() => ({ region: null }));

function CrashProbe({ region }: { region: string }) {
  const armed = useCrashProbe((s) => s.region === region);
  if (armed) throw new Error(`${PROBE_MESSAGE} ${region}`);
  return null;
}

const PROBE_MESSAGE = "OpenSpider crash test in region";

/**
 * Fehlergrenze je Region (Tabelle, Detail, Seitenleiste, Dialog, M2-ui-concept 2.6): zeigt eine Ersatzfläche mit
 * „Bereich neu laden“; der Fehler geht per `diag.logFrontend` ins Kern-Log und als lokaler Absturzbericht (OUT-246).
 */
export class RegionBoundary extends Component<{ name: string; children: ReactNode }, { failed: boolean }> {
  override state = { failed: false };

  static getDerivedStateFromError(): { failed: boolean } {
    return { failed: true };
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    void diag
      .logFrontend({ level: "error", message: error.message, context: { region: this.props.name, stack: info.componentStack ?? "" } })
      .catch(() => undefined);
    reportBoundaryError(this.props.name, error, info);
  }

  override render(): ReactNode {
    if (!this.state.failed) {
      return import.meta.env.DEV ? <><CrashProbe region={this.props.name} />{this.props.children}</> : this.props.children;
    }
    const t = getT();
    return (
      <div className="os-empty" role="alert" data-region-failed={this.props.name}>
        <p className="os-empty-text">{t("ui-region-failed")}</p>
        <Button onPress={() => this.setState({ failed: false })}>{t("ui-region-reload")}</Button>
      </div>
    );
  }
}

/** Fehlergrenze um die ganze Ansicht: fällt sie aus, entsteht ebenfalls ein `frontend`-Bericht (OUT-246). */
export class RootBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  override state = { failed: false };

  static getDerivedStateFromError(): { failed: boolean } {
    return { failed: true };
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    void diag
      .logFrontend({ level: "error", message: error.message, context: { region: "root", stack: info.componentStack ?? "" } })
      .catch(() => undefined);
    reportBoundaryError("root", error, info);
  }

  override render(): ReactNode {
    if (!this.state.failed) {
      return import.meta.env.DEV ? <><CrashProbe region="root" />{this.props.children}</> : this.props.children;
    }
    const t = getT();
    return (
      <div className="os-empty" role="alert" data-region-failed="root">
        <p className="os-empty-text">{t("ui-region-failed")}</p>
        <Button onPress={() => this.setState({ failed: false })}>{t("ui-region-reload")}</Button>
      </div>
    );
  }
}

/** Meldet unbehandelte Fehler im Fenster ans Kern-Log (`unhandledrejection`, `error`); kein Absturzbericht (OUT-246). */
export function installGlobalErrorLogging(win: Window = window): () => void {
  const log = (message: string) => void diag.logFrontend({ level: "error", message, context: null }).catch(() => undefined);
  const onRejection = (e: PromiseRejectionEvent) => log(String((e.reason as Error | undefined)?.message ?? e.reason));
  const onError = (e: ErrorEvent) => log(e.message);
  win.addEventListener("unhandledrejection", onRejection);
  win.addEventListener("error", onError);
  return () => {
    win.removeEventListener("unhandledrejection", onRejection);
    win.removeEventListener("error", onError);
  };
}
