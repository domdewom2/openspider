import { useEffect, useRef, useState } from "react";
import { useT } from "../../i18n";
import { mcp } from "../../ipc/mcp";
import type { McpClientKind } from "../../ipc/gen/McpClientKind";
import type { McpClientPreview } from "../../ipc/gen/McpClientPreview";
import { Banner, Button, Checkbox, Select } from "../../ui/primitives";
import type { McpDesktop } from "./desktop";
import { errorKey, redactText } from "./model";

export function ClientsPanel({ initialClient, desktop }: { initialClient: McpClientKind; desktop: McpDesktop }) {
  const t = useT();
  const [kind, setKind] = useState(initialClient);
  const [preview, setPreview] = useState<McpClientPreview | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [backup, setBackup] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const alive = useRef(true);
  const locked = useRef(false);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const run = async (action: () => Promise<void>) => {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError(null); setMessage(null);
    try { await action(); } catch (e) { if (alive.current) setError(errorKey(e)); }
    finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  const load = () => run(async () => {
    setPreview(null); setConfirmed(false); setBackup(null);
    const value = await mcp.clientConfigPreview({ client: kind });
    if (alive.current) setPreview(value);
  });
  return <section className="os-mcp-stack" aria-label={t("mcp-clients")}>
    <Select label={t("mcp-client")} value={kind} isDisabled={busy} onChange={value => {
      setKind(value as McpClientKind); setPreview(null); setConfirmed(false); setBackup(null); setMessage(null); setError(null);
    }} options={[
      { value: "claudeDesktop", label: "Claude Desktop" }, { value: "claudeCode", label: "Claude Code" }, { value: "codex", label: "Codex" },
    ]} />
    <Button isDisabled={busy} onPress={() => void load()}>{t("mcp-preview")}</Button>
    {preview && <>
      <p className="os-mcp-path">{redactText(preview.path)}</p>
      <pre className="os-mcp-preview" aria-label={t("mcp-diff")}>{redactText(preview.diff) || t("mcp-no-changes")}</pre>
      <details><summary>{t("mcp-config-content")}</summary><pre className="os-mcp-preview">{redactText(preview.after)}</pre></details>
      <Checkbox isSelected={confirmed} isDisabled={busy || !preview.diff} onChange={setConfirmed}>{t("mcp-confirm-write")}</Checkbox>
      <div className="os-mcp-actions">
        <Button isDisabled={busy || !confirmed || !preview.diff} onPress={() => void run(async () => {
          // Re-read immediately before writing: never confirm a diff that has changed on disk.
          const fresh = await mcp.clientConfigPreview({ client: kind });
          if (!alive.current) return;
          if (fresh.path !== preview.path || fresh.before !== preview.before || fresh.after !== preview.after) {
            setPreview(fresh); setConfirmed(false); setMessage("mcp-preview-changed"); return;
          }
          const result = await mcp.clientConfigWrite({ client: kind });
          if (alive.current) { setBackup(result.backupPath ?? null); setMessage("mcp-config-written"); setConfirmed(false); setPreview(null); }
        })}>{t("mcp-write")}</Button>
        <Button isDisabled={busy} onPress={() => void run(async () => {
          await desktop.copy(preview.after);
          if (alive.current) setMessage("mcp-copied");
        })}>{t("mcp-copy-config")}</Button>
      </div>
      <p>{t("mcp-backup-help")}</p>
    </>}
    {message && <Banner>{t(message)}</Banner>}
    {backup && <p className="os-mcp-path">{t("mcp-backup", { path: redactText(backup) })}</p>}
    {error && <Banner kind="error">{t(error)}</Banner>}
  </section>;
}
