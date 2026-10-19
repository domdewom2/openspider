import { useEffect, useState } from "react";
import { create } from "zustand";
import { useT } from "../../i18n";
import { mcp } from "../../ipc/mcp";
import type { McpClientKind } from "../../ipc/gen/McpClientKind";
import { registerAction, useAction } from "../../ui/actions";
import { registerMenuContributions } from "../../ui/menu/registry";
import { Banner, Button, Checkbox, Dialog, Menu, Tabs, TextField } from "../../ui/primitives";
import { AccessPanel } from "./AccessPanel";
import { ClientsPanel } from "./ClientsPanel";
import { LogPanel } from "./LogPanel";
import { ToolsPanel } from "./ToolsPanel";
import { nativeMcpDesktop, type McpDesktop } from "./desktop";
import { mcpMenu, popupEntries } from "./menu";
import { MCP_TABS, openMcp, parsePort, statusText, useMcpUi, type McpTab } from "./model";
import { useMcpController } from "./useMcpController";
import "./mcp.css";

// The status field and native menu share the same contribution model and actions.
const useMcpMenu = create<{ menu: ReturnType<typeof mcpMenu> }>(() => ({ menu: mcpMenu(false, false, false, false) }));
export function McpStatusField() {
  const t = useT();
  const status = useMcpUi(s => s.status);
  // Subscribe to host state changes that rebuild the popup's enabled/checked state.
  const menu = useMcpMenu(s => s.menu);
  return <span className="os-mcp-status" role="status" aria-live="polite">
    <Menu trigger={<Button variant="tool">{statusText(status, t)}</Button>} entries={popupEntries(menu, t)} label={t("mcp-title")} />
  </span>;
}

/** Main-window host: controls never depend on having an open crawl. */
export function McpHost({ desktop = nativeMcpDesktop }: { desktop?: McpDesktop }) {
  const t = useT();
  const controller = useMcpController();
  const { prefs, busy, available, error, setError, perform, changeServer, autostart } = controller;
  const tab = useMcpUi(s => s.tab);
  const status = useMcpUi(s => s.status);
  const [port, setPort] = useState("");
  const [clientKind, setClientKind] = useState<McpClientKind>("claudeDesktop");
  const [downloaded, setDownloaded] = useState(false);
  const running = status.kind !== "off";
  useEffect(() => { setPort(prefs?.port == null ? "" : String(prefs.port)); }, [prefs?.port]);
  const start = () => {
    const parsed = parsePort(port);
    if (parsed === undefined) { setError("mcp-port-error"); openMcp("server"); return; }
    if (available && !running) void changeServer(parsed);
  };
  const download = () => void perform(async () => {
    setDownloaded(false);
    const path = await desktop.chooseMarkdown();
    if (path === null) return;
    await mcp.saveToolsMarkdown({ path });
    setDownloaded(true);
  });
  useAction("mcp.start", start);
  useAction("mcp.stop", () => { if (available && running) void changeServer(null, true); });
  useAction("mcp.autostart", () => { if (available) void autostart(!prefs?.autostart); });
  useAction("mcp.markdown", download);
  useEffect(() => {
    const offs = MCP_TABS.map(id => registerAction(id === "server" ? "mcp.configure" : `mcp.${id}`, () => openMcp(id)));
    for (const kind of ["claudeDesktop", "claudeCode", "codex"] as const) offs.push(registerAction(`mcp.${kind}`, () => {
      setClientKind(kind); openMcp("clients");
    }));
    return () => offs.reverse().forEach(off => off());
  }, []);
  useEffect(() => {
    const menu = mcpMenu(running, prefs?.autostart ?? false, busy, available);
    useMcpMenu.setState({ menu });
    return registerMenuContributions("features/mcp", menu);
  }, [running, prefs?.autostart, busy, available]);
  const close = () => { useMcpUi.setState({ tab: null }); setDownloaded(false); };
  const serverPanel = <section className="os-mcp-stack" aria-label={t("mcp-server")}>
    <p>{statusText(status, t)}</p>
    <p>{t("mcp-loopback")}</p>
    <TextField label={t("mcp-port")} value={port} onChange={setPort} inputMode="numeric" isDisabled={busy || running}
      description={t("mcp-port-help")} error={parsePort(port) === undefined ? t("mcp-port-error") : null} />
    <Checkbox isSelected={prefs?.autostart ?? false} isDisabled={busy || !available} onChange={value => void autostart(value)}>{t("mcp-autostart")}</Checkbox>
    <div className="os-mcp-actions">
      <Button variant="primary" isDisabled={busy || !available || running || parsePort(port) === undefined} onPress={start}>{t("mcp-start")}</Button>
      <Button isDisabled={busy || !available || !running} onPress={() => void changeServer(null, true)}>{t("mcp-stop")}</Button>
      <Button isDisabled={busy} onPress={() => { setError(null); void controller.refresh(); }}>{t("mcp-refresh")}</Button>
    </div>
  </section>;
  return <>
    {!tab && error && <div className="os-mcp-notice"><Banner kind="error" onDismiss={() => setError(null)} dismissLabel={t("ui-dialog-close")}>{t(error)}</Banner></div>}
    {!tab && downloaded && <div className="os-mcp-notice"><Banner onDismiss={() => setDownloaded(false)} dismissLabel={t("ui-dialog-close")}>{t("mcp-markdown-saved")}</Banner></div>}
    {tab && <Dialog title={t("mcp-title")} isOpen onOpenChange={open => { if (!open) close(); }} size="large"
      footer={<div className="os-dialog-footer"><Button onPress={close}>{t("ui-dialog-close")}</Button></div>}>
      <div className="os-mcp-stack">
        {error && <Banner kind="error" onDismiss={() => setError(null)} dismissLabel={t("ui-dialog-close")}>{t(error)}</Banner>}
        {downloaded && <Banner>{t("mcp-markdown-saved")}</Banner>}
        <Tabs label={t("mcp-title")} selected={tab} onSelect={id => openMcp(id as McpTab)} tabs={MCP_TABS.map(id => ({
          id, label: t(`mcp-${id}`), content: id === tab ? (
            id === "server" ? serverPanel : id === "tools" ? <ToolsPanel controller={controller} download={download} /> :
            id === "access" ? <AccessPanel desktop={desktop} /> : id === "clients" ? <ClientsPanel key={clientKind} initialClient={clientKind} desktop={desktop} /> : <LogPanel />
          ) : undefined,
        }))} />
      </div>
    </Dialog>}
  </>;
}
