// Harness für visuelle und Zugänglichkeitstests (nur Dev-Build): deterministische Szenen ohne Kern, gesteuert über die
// Adresse – /?harness=<szene>&theme=light|dark&lang=de|en&density=compact|normal&platform=mac|win&zoom=100&accent=%23rrggbb
// Szenen: primitives, empty, settings, shortcuts.
import { Serp } from "../features/detail/serp/Serp";
import { ViewSource } from "../features/detail/source/ViewSource";
import { fachFixtureCore } from "../features/detail/rendered/fixtures/core";
import { CRAWL_ID } from "../features/detail/rendered/fixtures/data";
import type { DetailPanelProps } from "../features/detail/frame/types";
import { WordCloudView } from "../features/viz/WordCloudView";
import { fxUiWords } from "../features/viz/fixtures/synthetic";
import { startAppearanceSync } from "../app/appearance";
import { App, registerMainRegions } from "../App";
import { bootWindow } from "../app/runtime";
import { TopTabs } from "../features/toptabs/TopTabs";
import { UpperTable } from "../features/table/UpperTable";
import { DetailFrame } from "../features/detail/frame/DetailFrame";
import shellCatalog from "./fixtures/fx-ui-basic-catalog.json";
import { tableFixture } from "../features/table/fixtures/core";
import "../app/app.css";
import type { Root } from "react-dom/client";
import { useState } from "react";
import { SettingsFrame } from "../features/settings/frame/SettingsFrame";
import { createI18n, i18n, loadFtlFiles, setLanguage, useFormat, useT } from "../i18n";
import { createClient, setClient } from "../ipc/client";
import { MockCore } from "../ipc/transport/mock";
import { usePrefs } from "../state/prefs";
import { setPlatformOverride } from "../ui/platform";
import { EMPTY_CODES, type EmptyState as EmptyStateDto } from "../ui/emptyState/codes";
import { SHORTCUTS, shortcutTable } from "../ui/shortcuts";
import { currentPlatform } from "../ui/platform";
import {
  Badge,
  Banner,
  Button,
  Checkbox,
  ColorPicker,
  Dialog,
  DialogFooter,
  EmptyState,
  KeyHint,
  Menu,
  ProgressBar,
  PropertyTable,
  RadioGroup,
  SearchField,
  Select,
  SplitPane,
  Switch,
  Tabs,
  TextField,
  Tree,
} from "../ui/primitives";
import { IssueTypeIcon } from "../ui/icons";
import { applyAppearance, type AppearanceState } from "../ui/tokens/theme";
import { DEFAULT_APPEARANCE } from "../ui/tokens/theme";
import { SEGMENT_PALETTE } from "../ui/tokens/tokens";
import "../ui/ui.css";
import "./harness.css";

const SAMPLE_ARG: Record<string, string> = { FEATURE_DISABLED: "cookies", API_NOT_CONNECTED: "gsc", NEEDS_CRAWL_ANALYSIS: "canonicals" };

function Primitives() {
  const t = useT();
  const f = useFormat();
  const [text, setText] = useState("https://example.com/");
  const [sw, setSw] = useState(true);
  const [radio, setRadio] = useState("a");
  const [sel, setSel] = useState("100");
  const [tab, setTab] = useState("one");
  const [size, setSize] = useState(160);
  const [open, setOpen] = useState(false);
  const [accent, setAccent] = useState("#2f855a");
  return (
    <div className="harness-grid">
      <section aria-label="buttons">
        <Button variant="primary">{t("ui-dialog-ok")}</Button> <Button>{t("ui-dialog-cancel")}</Button> <Button variant="danger">{t("ui-dialog-reset")}</Button>{" "}
        <Button isDisabled>{t("ui-dialog-close")}</Button>
      </section>
      <section aria-label="fields">
        <TextField label="URL" value={text} onChange={setText} />
        <TextField label="URL" value="http://" onChange={() => undefined} error={t("color-invalid")} />
        <SearchField label={t("ui-search-placeholder")} value="" onChange={() => undefined} clearLabel={t("ui-clear-search")} placeholder={t("ui-search-placeholder")} />
        <Switch isSelected={sw} onChange={setSw}>{t("settings-general-notifications")}</Switch>
        <Checkbox isSelected={sw} onChange={setSw}>{t("settings-appearance-zebra")}</Checkbox>
        <RadioGroup label={t("settings-appearance-theme")} value={radio} onChange={setRadio} options={[{ value: "a", label: t("settings-appearance-theme-light") }, { value: "b", label: t("settings-appearance-theme-dark") }]} />
        <Select label={t("settings-appearance-zoom")} value={sel} onChange={setSel} options={[{ value: "100", label: "100 %" }, { value: "125", label: "125 %" }]} />
        <ColorPicker label={t("settings-appearance-accent")} value={accent} onChange={setAccent} theme={document.documentElement.dataset.theme === "dark" ? "dark" : "light"} />
      </section>
      <section aria-label="navigation">
        <Tabs label="demo" selected={tab} onSelect={setTab} tabs={[{ id: "one", label: "One", content: <p>1</p> }, { id: "two", label: "Two", content: <p>2</p> }]} />
        <div style={{ height: 80, border: "1px solid var(--os-border)" }}>
          <SplitPane orientation="horizontal" size={size} onSizeChange={setSize} label="demo" first={<p>A</p>} second={<p>B</p>} />
        </div>
        <Tree label="demo" nodes={[{ id: "a", label: "Overview", textValue: "Overview", children: [{ id: "a1", label: "Internal", textValue: "Internal" }] }]} />
        <Menu label="demo" trigger={<Button>Menu</Button>} entries={[{ id: "one", label: "One", shortcut: "⌘1" }, { id: "two", label: "Two", isDisabled: true, disabledReason: "…" }]} />
      </section>
      <section aria-label="feedback">
        <Banner kind="info">{t("ui-region-failed")}</Banner>
        <Banner kind="warning">{t("color-low-contrast", { ratio: "1.2" })}</Banner>
        <Banner kind="error">{t("err-code-io")}</Banner>
        <ProgressBar label="p" value={40} />
        <ProgressBar label="p" />
        <Badge tone="accent">{f.integer(1234567)}</Badge> <Badge>{f.decimal(1234.5)}</Badge>
        <IssueTypeIcon type="issue" /> <IssueTypeIcon type="warning" /> <IssueTypeIcon type="opportunity" />
        <PropertyTable caption="demo" nameHeader="Name" valueHeader="Value" rows={[{ name: "Status", value: "200" }, { name: "Size", value: f.bytes(1536000) }]} />
        <div className="harness-segments">{SEGMENT_PALETTE.map((c, i) => <span key={i} style={{ background: document.documentElement.dataset.theme === "dark" ? c.dark : c.light }} />)}</div>
        <p><KeyHint action="crawl.pause" /> <KeyHint combo="Cmd+Alt+Shift+X" /></p>
        <Button onPress={() => setOpen(true)}>Dialog</Button>
        <Dialog title="Dialog" isOpen={open} onOpenChange={setOpen} footer={(close) => <DialogFooter okLabel={t("ui-dialog-ok")} cancelLabel={t("ui-dialog-cancel")} onOk={close} onCancel={close} />}>
          <p>{t("ui-region-failed")}</p>
        </Dialog>
      </section>
    </div>
  );
}

function EmptyStates() {
  return (
    <div className="harness-empties">
      {EMPTY_CODES.map((code) => {
        const arg = SAMPLE_ARG[code];
        const state: EmptyStateDto = { code, ...(arg ? { arg } : {}), action: code === "NO_MATCH" ? { kind: "clearSearch" } : code === "NEEDS_CRAWL_ANALYSIS" ? { kind: "startAnalysis" } : null };
        return <EmptyState key={code} state={state} onAction={() => undefined} />;
      })}
    </div>
  );
}

function Shortcuts() {
  const t = useT();
  const platform = currentPlatform();
  const rows = shortcutTable(platform, document.documentElement.lang);
  return (
    <table className="os-proptable" data-count={SHORTCUTS.length}>
      <tbody>
        {rows.map((r) => (
          <tr key={r.id}>
            <th scope="row">{t(r.labelKey)}</th>
            <td>{r.keys.join(" / ")}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

const detailProps: DetailPanelProps = {
  selection: { urlIds: [1], focus: 1, crawlId: CRAWL_ID }, rendering: true, htmlStored: true,
  storeUncrawlable: false, positions: [], listMode: false,
  actions: { navigate: () => undefined, back: () => undefined, openBrowser: () => undefined, export: () => undefined, reportError: error => { throw error; } },
};
function Scene({ scene }: { scene: string }) {
  switch (scene) {
    case "serp": return <Serp {...detailProps} />;
    case "source": return <ViewSource {...detailProps} />;
    case "viz": return <WordCloudView snapshotId="u32-words" urlId={1} generation={1} />;
    case "empty":
      return <EmptyStates />;
    case "community":
    case "settings":
      return <div style={{ height: "100vh" }}><SettingsFrame {...(location.hash ? { anchor: decodeURIComponent(location.hash.slice(1)) } : {})} /></div>;
    case "shortcuts":
      return <Shortcuts />;
    default:
      return <Primitives />;
  }
}

export function mountHarness(root: Root): void {
  const p = new URLSearchParams(location.search);
  const scene = p.get("harness") || "primitives";
  if (scene === "community") {
    const resources = import.meta.glob<string>("/e2e/visual/fixtures/community/i18n/*.ftl", { query: "?raw", import: "default", eager: true });
    Object.assign(i18n, createI18n([...loadFtlFiles(), ...loadFtlFiles(resources)]));
  }
  const platform = p.get("platform") === "mac" ? "mac" : "win";
  setPlatformOverride(platform);
  const lang = p.get("lang") === "de" ? "de" : "en";
  const appearance: AppearanceState = {
    ...DEFAULT_APPEARANCE,
    theme: p.get("theme") === "dark" ? "dark" : "light",
    density: p.get("density") === "normal" ? "normal" : "compact",
    accent: p.get("accent") ?? DEFAULT_APPEARANCE.accent,
    zebra: p.get("zebra") !== "off",
  };
  // Feste Einstellungen über den Mock-Kern, damit die Szenen deterministisch sind (kein localStorage, kein Netz).
  const core = scene === "shell" ? tableFixture().core : ["serp", "source"].includes(scene) ? fachFixtureCore().core : new MockCore();
  if (scene === "viz") core.handlers.set("viz.words", () => fxUiWords());
  if (scene === "shell") {
    core.handlers.set("catalog.tabs", () => shellCatalog);
    core.handlers.set("catalog.filterCounts", () => ({ version: 1, filters: shellCatalog.flatMap(tab => tab.filters.map(filter => ({ tab: tab.id, filter: { ...filter, count: filter.id === "All" ? 3 : 0 } }))) }));
    registerMainRegions({ tabs: TopTabs, table: UpperTable, detail: DetailFrame });
  }
  setClient(createClient(core.transport("harness")));
  void setLanguage(lang).then(async () => {
    if (scene === "shell") await bootWindow({ kind: "main", transport: core.transport("main") }).ready;
    else await usePrefs.getState().load();
    usePrefs.setState({ appearance });
    if (scene === "community") startAppearanceSync({ platform });
    applyAppearance(document.documentElement, appearance, { platform, systemDark: false });
    root.render(
      <div className={scene === "shell" ? "harness-shell" : "harness-root"} data-harness-ready={scene}>
        {scene === "shell" ? <App /> : <Scene scene={scene} />}
      </div>,
    );
  });
}
