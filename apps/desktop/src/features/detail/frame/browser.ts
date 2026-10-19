import { client } from "../../../ipc/client";

export const DETAIL_OPEN_BROWSER_EVENT = "os:detail-open-browser";
/** U10/N01 supplies actions.openBrowser or handles this request with its native launchBrowser. */
export function requestBrowser(address: string): void {
  if (!/^https?:\/\//i.test(address)) return;
  const request = new CustomEvent<{ address: string }>(DETAIL_OPEN_BROWSER_EVENT, { detail: { address }, cancelable: true });
  document.dispatchEvent(request);
  // Browser fixtures and the bridge can use a popup; a Tauri WebView must use the native handoff.
  if (!request.defaultPrevented && client.transportName !== "tauri") window.open(address, "_blank", "noopener,noreferrer");
}
