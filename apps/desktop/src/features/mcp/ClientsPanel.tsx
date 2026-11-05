import { useCallback, useEffect, useRef, useState } from "react";
import { useT } from "../../i18n";
import { mcp } from "../../ipc/mcp";
import type { McpClientKind } from "../../ipc/gen/McpClientKind";
import type { McpClientPreview } from "../../ipc/gen/McpClientPreview";
import { toCmdError } from "../../ipc/errors";
import { Banner, Button, Checkbox, Select } from "../../ui/primitives";
import type { McpDesktop } from "./desktop";
import { errorKey, redactText } from "./model";

export const MCP_PROGRAMS: Record<McpClientKind, string> = {
  claudeDesktop: "Claude Desktop",
  claudeCode: "Claude Code",
  codex: "Codex",
  cursor: "Cursor",
};

/** Programs whose completion message asks to reload the MCP servers instead of starting a new session
 * (AGENT-043, addendum 2026-11-05). */
const RELOAD_PROGRAMS = new Set<McpClientKind>(["cursor"]);

/** Fixed messages the service may report for client files (AGENT-043); details are never rendered. */
const CLIENT_ERRORS = new Set(["mcp-client-changed", "mcp-client-problem-syntax", "mcp-client-problem-unsupported", "mcp-client-problem-symlink", "mcp-client-problem-readOnly"]);
type Message = { key: string; args?: Record<string, string | number> };
type Done = { kind: "connected" | "unchanged" | "disconnected"; backup: string | null };

function clientError(e: unknown): Message {
  const error = toCmdError(e);
  if (CLIENT_ERRORS.has(error.messageKey)) {
    const line = Number(error.args?.line);
    return { key: error.messageKey, args: Number.isFinite(line) ? { line } : undefined };
  }
  return { key: errorKey(e) };
}

/** One-click connection for one program (AGENT-043, user decision 2026-11-04): backup, own entry and the
 * `crawl` class in a single step. The service only returns masked, changed lines, never the foreign file. */
export function ConnectPanel({ client, desktop }: { client: McpClientKind; desktop: McpDesktop }) {
  const t = useT();
  const program = MCP_PROGRAMS[client];
  const [preview, setPreview] = useState<McpClientPreview | null>(null);
  const [allowCrawl, setAllowCrawl] = useState(true);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<Done | null>(null);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<Message | null>(null);
  const alive = useRef(true);
  const locked = useRef(false);
  const touched = useRef(false);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const load = useCallback(async () => {
    const value = await mcp.clientConfigPreview({ client });
    if (!alive.current) return;
    setPreview(value);
    // Default on for a new connection; an existing one shows the current grant.
    if (!touched.current) setAllowCrawl(value.state === "notConnected" ? true : value.crawlAllowed);
  }, [client]);
  useEffect(() => { void load().catch(e => { if (alive.current) setError(clientError(e)); }); }, [load]);
  const run = async (action: () => Promise<void>) => {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError(null); setDone(null); setCopied(false);
    try { await action(); } catch (e) { if (alive.current) setError(clientError(e)); }
    finally {
      locked.current = false;
      if (alive.current) setBusy(false);
      await load().catch(() => undefined);
    }
  };
  const connect = () => void run(async () => {
    const result = await mcp.clientConnect({ client, allowCrawl });
    if (alive.current) setDone({ kind: result.changed ? "connected" : "unchanged", backup: result.backupPath ?? null });
  });
  const disconnect = () => void run(async () => {
    const result = await mcp.clientDisconnect({ client });
    if (alive.current) setDone({ kind: "disconnected", backup: result.backupPath ?? null });
  });
  const copy = () => void run(async () => {
    if (!preview) return;
    await desktop.copy(preview.entry);
    if (alive.current) setCopied(true);
  });
  if (!preview) return <section className="os-mcp-stack" aria-label={t("mcp-connect-title", { program })}>
    {error ? <Banner kind="error">{t(error.key, error.args)}</Banner> : <p role="status">{t("mcp-loading")}</p>}
  </section>;
  const blocked = preview.problem != null;
  const connected = preview.state !== "notConnected";
  const state = blocked ? "mcp-client-state-blocked" : `mcp-client-state-${preview.state}`;
  const diff = redactText(preview.connectDiff);
  return <section className="os-mcp-stack" aria-label={t("mcp-connect-title", { program })} aria-busy={busy}>
    <p className="os-mcp-connect-state"><strong>{t(state)}</strong></p>
    <p className="os-mcp-path">{t("mcp-client-file", { path: redactText(preview.path) })}</p>
    {preview.extraKeys.length > 0 && <Banner kind="warning">{t("mcp-client-extra-keys", { keys: preview.extraKeys.map(redactText).join(", ") })}</Banner>}
    {preview.problem && <Banner kind="error">{t(`mcp-client-problem-${preview.problem}`, { line: preview.line ?? 0 })}</Banner>}
    <div className="os-mcp-connect-grant">
      <Checkbox isSelected={allowCrawl} isDisabled={busy || blocked} onChange={value => { touched.current = true; setAllowCrawl(value); }}>{t("mcp-allow-crawl")}</Checkbox>
      <p className="os-mcp-hint">{t("mcp-allow-crawl-help")}</p>
    </div>
    <div className="os-mcp-actions">
      <Button variant="primary" isDisabled={busy || blocked} onPress={connect}>{t(connected ? "mcp-update" : "mcp-connect")}</Button>
      {connected && <Button isDisabled={busy || blocked} onPress={disconnect}>{t("mcp-disconnect")}</Button>}
      <Button isDisabled={busy} onPress={copy}>{t("mcp-copy-entry")}</Button>
    </div>
    {done && <Banner>
      <p>{t(`mcp-client-done-${done.kind}${RELOAD_PROGRAMS.has(client) ? "-reload" : ""}`, { program })}</p>
      {done.backup && <p className="os-mcp-path">{t("mcp-backup", { path: redactText(done.backup) })}</p>}
    </Banner>}
    {copied && <Banner>{t("mcp-entry-copied")}</Banner>}
    {error && <Banner kind="error">{t(error.key, error.args)}</Banner>}
    {!blocked && <details className="os-mcp-details">
      <summary>{t("mcp-show-changes")}</summary>
      <p className="os-mcp-hint">{t("mcp-changes-help")}</p>
      <pre className="os-mcp-preview" aria-label={t("mcp-diff")}>{diff || t(connected ? "mcp-no-changes" : "mcp-loading")}</pre>
    </details>}
  </section>;
}

/** Tab „Verbundene Programme“: the same panel with a program selection. */
export function ClientsPanel({ initialClient, desktop }: { initialClient: McpClientKind; desktop: McpDesktop }) {
  const t = useT();
  const [kind, setKind] = useState(initialClient);
  return <section className="os-mcp-stack" aria-label={t("mcp-clients")}>
    <Select label={t("mcp-client")} value={kind} onChange={value => setKind(value as McpClientKind)}
      options={(Object.keys(MCP_PROGRAMS) as McpClientKind[]).map(value => ({ value, label: MCP_PROGRAMS[value] }))} />
    <ConnectPanel key={kind} client={kind} desktop={desktop} />
  </section>;
}
