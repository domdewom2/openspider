// Loaded explicitly by the native E2E runner through Vite; no production entry imports this module.
import { client } from "../ipc/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { copyDesktopText, sendDesktopNotification } from "../ipc/transport/tauri";
export { copyDesktopText, sendDesktopNotification };
export const notificationPermission = () => client.invoke<boolean | null>("notification.is_permission_granted");
export async function dpi() {
  return { native: await getCurrentWindow().scaleFactor(), dom: devicePixelRatio,
    canvases: [...document.querySelectorAll("canvas")].filter(c => c.clientWidth > 0 && c.clientHeight > 0).map(c => ({ backing: c.width, css: c.clientWidth, backingHeight: c.height, cssHeight: c.clientHeight })) };
}
