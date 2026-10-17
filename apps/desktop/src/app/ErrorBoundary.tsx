import { Component, type ErrorInfo, type ReactNode } from "react";
import { diag } from "../ipc/diag";
import { getT } from "../i18n";
import { Button } from "../ui/primitives/Button";

/**
 * Fehlergrenze je Region (Tabelle, Detail, Seitenleiste, Dialog, M2-ui-concept 2.6): zeigt eine Ersatzfläche mit
 * „Bereich neu laden“; der Fehler geht per `diag.logFrontend` ins Kern-Log.
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
  }

  override render(): ReactNode {
    if (!this.state.failed) return this.props.children;
    const t = getT();
    return (
      <div className="os-empty" role="alert" data-region-failed={this.props.name}>
        <p className="os-empty-text">{t("ui-region-failed")}</p>
        <Button onPress={() => this.setState({ failed: false })}>{t("ui-region-reload")}</Button>
      </div>
    );
  }
}

/** Meldet unbehandelte Fehler im Fenster ans Kern-Log (`unhandledrejection`, `error`). */
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
