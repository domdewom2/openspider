import { getCurrentWindow } from "@tauri-apps/api/window";
import { createRoot } from "react-dom/client";
import { createClient, setClient } from "../../src/ipc/client";
import { createTauriTransport } from "../../src/ipc/transport/tauri";
import { setLanguage } from "../../src/i18n";
import { AccessPanel } from "../../src/features/mcp/AccessPanel";
import { nativeMcpDesktop } from "../../src/features/mcp/desktop";
import "../../src/ui/ui.css";

// E2E-B: real WKWebView and clipboard plugin. Only the absent K17 token response is delayed.
const transport = createTauriTransport();
setClient(createClient({ ...transport, invoke: async <T,>(command: string, args: unknown) => {
  if (command === "mcp.token") {
    await new Promise(resolve => setTimeout(resolve, 1_500));
    return { masked: "••••", value: "u25-native-delayed-token" } as T;
  }
  return transport.invoke<T>(command, args);
} }));
await setLanguage("en");
await getCurrentWindow().show();
await getCurrentWindow().setFocus();
window.mcpNative = { restore: (text: string) => nativeMcpDesktop.copy(text) };
declare global { interface Window { mcpNative: { restore(text: string): Promise<void> } } }
createRoot(document.getElementById("root")!).render(<AccessPanel desktop={nativeMcpDesktop} />);
