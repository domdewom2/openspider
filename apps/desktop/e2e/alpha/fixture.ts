// Fixture der Alpha-Abnahme (M2-A01): je Test eine echte Bridge mit eigenem Home, je Worker ein Fixture-Server
// (fx-ui-basic). Jede Seite wird auf Konsolenfehler überwacht (Alpha-Kriterium 7).
import { test as base, expect as baseExpect, type Locator, type Page } from "@playwright/test";
import { ensureTools, startAlphaBridge, startSite, type AlphaBridge, type Site } from "./support";

// Der Vite-Entwicklungsserver übersetzt Module beim ersten Zugriff; unter paralleler Last braucht das länger als die 5 s der Vorgabe.
export const expect = baseExpect.configure({ timeout: 15_000 });

export interface GridWindow {
  headers: string[];
  /** Sichtbarer Ausschnitt der Tabelle (virtualisiert); jede Zeile mit `aria-rowindex` und Auswahlstatus. */
  rows: { index: number; selected: boolean; cells: string[] }[];
}

export interface Alpha {
  site: Site;
  bridge: AlphaBridge;
  /** Hauptfenster der App (Vite-Frontend, IPC über die Bridge). */
  page: Page;
  /** Beobachtet eine weitere Seite (z. B. das Einstellungsfenster) auf Konsolenfehler. */
  track(page: Page): void;
  /** Öffnet das Hauptfenster gegen die Bridge und wartet auf die fertige Hülle. */
  open(params?: Record<string, string>): Promise<void>;
  /** Öffnet das Einstellungsfenster in einer zweiten Seite desselben Kontexts (gleiche Bridge). */
  openSettings(): Promise<Page>;
  /** Start-URL des Fixture-Servers. */
  startUrl: string;
  /**
   * Speichert ein Profil (Abschnitt für Abschnitt über die Werkseinstellungen gelegt) und macht es zum Standard. Die Kopfleiste
   * startet Crawls ohne Profilangabe, also mit dem Standardprofil. `config.set` ohne offenen Crawl genügt dafür nicht (Befund A01-04).
   */
  useProfile(name: string, sections: Record<string, Record<string, unknown>>): Promise<void>;
  /** Drosselt Crawls auf `rate` URL/s mit einem Thread, damit Pause, Fortsetzen und Live-Verhalten beobachtbar sind. */
  slowCrawl(rate?: number, sections?: Record<string, Record<string, unknown>>): Promise<void>;
  /** Trägt die Start-URL ein und startet mit Enter (UI-003). */
  startFromHeader(url?: string): Promise<void>;
  /** Wartet, bis die Statusleiste „Completed“ zeigt. */
  waitCompleted(timeout?: number): Promise<void>;
  /** Generischer IPC-Aufruf aus der Seite (derselbe Client wie die Oberfläche). */
  invoke<T = unknown>(cmd: string, args?: unknown): Promise<T>;
  /** Der obere Tabellenbereich. */
  table(): Locator;
  /** Text des Zählers „Showing: N of M“. */
  counter(): Promise<string>;
  gridWindow(): Promise<GridWindow>;
  /** Wie `gridWindow`, wartet aber, bis keine Platzhalterzellen („…“, Zeilen werden nachgeladen) mehr sichtbar sind. */
  settledGrid(): Promise<GridWindow>;
  /** Wechselt den oberen Tab. */
  selectTab(name: string): Promise<void>;
  /**
   * Wechselt den Tab eines fertigen Crawls und wartet auf geladene Zeilen. Gelegentlich bleibt die Tabelle nach dem Wechsel direkt nach
   * dem Crawl-Ende leer („Showing: 0 of N“, Befund A01-21); dann wird über einen anderen Tab erneut gewechselt. Jeder solche Fall wird
   * als Annotation `table-reload` am Test vermerkt.
   */
  openTable(name: string): Promise<void>;
  /** Überschreibt den nativen Zielordner-/Datei-Dialog des Exports (im Browser nicht vorhanden). */
  chooseExportPath(resolve: string): Promise<void>;
  consoleErrors: string[];
}

const READY = "[data-shell-ready]";

export const test = base.extend<{ alpha: Alpha }, { site: Site }>({
  site: [
    async ({}, use) => {
      await ensureTools();
      const site = await startSite();
      await use(site);
      await site.stop();
    },
    { scope: "worker" },
  ],

  alpha: async ({ page, context, site }, use, testInfo) => {
    const bridge = await startAlphaBridge();
    const consoleErrors: string[] = [];
    const track = (p: Page) => {
      p.on("console", (m) => {
        if (m.type() === "error") consoleErrors.push(`console.error: ${m.text()}`);
      });
      p.on("pageerror", (e) => consoleErrors.push(`pageerror: ${e.message}`));
    };
    track(page);
    const q = (extra: Record<string, string> = {}) => new URLSearchParams({ ...extra, bridge: bridge.url }).toString();
    const invoke = <T,>(cmd: string, args?: unknown): Promise<T> =>
      page.evaluate(
        async ({ cmd, args }) => {
          const path = "/src/ipc/client.ts";
          const { client } = await import(/* @vite-ignore */ path);
          return client.invoke(cmd, args);
        },
        { cmd, args },
      ) as Promise<T>;
    const table = () => page.getByRole("region", { name: /^(Crawl table|Crawl-Tabelle)$/ });
    const alpha: Alpha = {
      site,
      bridge,
      page,
      track,
      consoleErrors,
      startUrl: `${site.base}/`,
      async open(params) {
        await page.goto(`/?${q(params)}`);
        await expect(page.locator(READY)).toHaveAttribute("data-shell-ready", "true");
      },
      async openSettings() {
        const settings = await context.newPage();
        track(settings);
        await settings.goto(`/settings.html?${q()}`);
        return settings;
      },
      async useProfile(name, sections) {
        await page.evaluate(
          async ({ name, sections }) => {
            const configPath = "/src/ipc/config.ts";
            const profilePath = "/src/ipc/profile.ts";
            const { config } = await import(/* @vite-ignore */ configPath);
            const { profile } = await import(/* @vite-ignore */ profilePath);
            const dto = await config.get();
            const root = dto.config as Record<string, Record<string, unknown>>;
            const next: Record<string, unknown> = { ...root };
            for (const [key, value] of Object.entries(sections)) next[key] = { ...(root[key] ?? {}), ...value };
            await profile.save({ name, overwrite: true, config: { ...dto, config: next } });
            await profile.setDefault({ name });
          },
          { name, sections },
        );
      },
      async slowCrawl(rate = 3, sections = {}) {
        await alpha.useProfile("alpha-slow", { ...sections, speed: { threads: 1, max_urls_per_sec: rate, ...(sections.speed ?? {}) } });
      },
      async startFromHeader(url) {
        const field = page.getByLabel("Start URL");
        await field.fill(url ?? `${site.base}/`);
        await field.press("Enter");
      },
      async waitCompleted(timeout = 30_000) {
        await expect(page.getByTestId("crawl-phase")).toHaveText("Completed", { timeout });
      },
      invoke,
      table,
      async counter() {
        return table().getByText(/^(Showing|Angezeigt): /).innerText();
      },
      async gridWindow() {
        return page.evaluate(() => {
          const root = document.querySelector(".os-grid-a11y");
          const rows = [...(root?.querySelectorAll("[role=row]") ?? [])] as HTMLElement[];
          const headerRow = rows.find((r) => r.getAttribute("aria-rowindex") === "1");
          return {
            headers: [...(headerRow?.querySelectorAll("[role=columnheader]") ?? [])].map((c) => c.textContent ?? ""),
            rows: rows
              .filter((r) => r !== headerRow)
              .map((r) => ({
                index: Number(r.getAttribute("aria-rowindex")),
                selected: r.getAttribute("aria-selected") === "true",
                cells: [...r.querySelectorAll("[role=gridcell]")].map((c) => c.textContent ?? ""),
              })),
          };
        });
      },
      async settledGrid() {
        let last: GridWindow = { headers: [], rows: [] };
        await expect
          .poll(async () => {
            last = await alpha.gridWindow();
            return last.rows.length > 0 && last.rows.every((r) => r.cells.every((c) => c !== "…"));
          }, { timeout: 10_000 })
          .toBe(true);
        return last;
      },
      async openTable(name) {
        const other = name === "External" ? "Response Codes" : "External";
        for (let attempt = 0; attempt < 4; attempt++) {
          await alpha.selectTab(name);
          const loaded = await expect
            .poll(async () => !/^(Showing|Angezeigt): 0 (of|von) /.test(await alpha.counter().catch(() => "Showing: 0 of –")), { timeout: 4000 })
            .toBe(true)
            .then(() => true, () => false);
          if (loaded) return;
          testInfo.annotations.push({ type: "table-reload", description: `${name}: Tabelle blieb leer (Versuch ${attempt + 1})` });
          await alpha.selectTab(other);
        }
        await alpha.selectTab(name);
        await expect.poll(() => alpha.counter(), { timeout: 10_000 }).not.toMatch(/^(Showing|Angezeigt): 0 (of|von) /);
      },
      async selectTab(name) {
        await page.getByRole("tab", { name, exact: true }).click();
      },
      async chooseExportPath(resolve) {
        await page.evaluate(async (base) => {
          const path = "/src/features/export/native.ts";
          const { nativeExportDesktop } = await import(/* @vite-ignore */ path);
          nativeExportDesktop.choose = async (o: { folder: boolean; defaultName: string }) => `${base}/${o.folder ? "folder" : o.defaultName}`;
          nativeExportDesktop.directory = async (p: string, folder: boolean) => (folder ? p : p.slice(0, p.lastIndexOf("/")));
          nativeExportDesktop.reveal = async (p: string) => {
            (window as unknown as { __revealed?: string[] }).__revealed = [...((window as unknown as { __revealed?: string[] }).__revealed ?? []), p];
          };
        }, resolve);
      },
    };
    await use(alpha);
    // Fehlermeldungen werden vor dem Beenden der Bridge festgehalten: WebKit meldet beim Abbruch der Verbindung einen Konsolenfehler.
    const errors = [...consoleErrors];
    await bridge.stop();
    expect(errors, "Konsolenfehler im Hauptfenster oder Einstellungsfenster (Alpha-Kriterium 7)").toEqual([]);
  },
});
