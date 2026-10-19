import { createRoot } from "react-dom/client";
import { bootWindow } from "../../src/app/runtime";
import { setLanguage } from "../../src/i18n";
import { client } from "../../src/ipc/client";
import { useSession } from "../../src/state/session";
import { CrawlsHost, openCrawls } from "../../src/features/crawls";
import { nativeCrawlDesktop, type CrawlDesktop } from "../../src/features/crawls/desktop";
import "../../src/ui/ui.css";
const runtime = bootWindow({ kind: "main", menu: false });
await runtime.ready;
await setLanguage("en");
let destination: string | null = null;
const desktop: CrawlDesktop = { ...nativeCrawlDesktop, chooseSave: async () => destination, chooseDirectory: async () => destination };
const api = {
  invoke: (command: string, args?: unknown) => client.invoke(command, args),
  phase: () => useSession.getState().crawl?.phase,
  setDestination(value: string) { destination = value; },
};
declare global { interface Window { crawlsBridge: typeof api } }
window.crawlsBridge = api;
createRoot(document.getElementById("root")!).render(<main><button onClick={e => openCrawls(e.currentTarget)}>Manage</button><CrawlsHost desktop={desktop} /></main>);
