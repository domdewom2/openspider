import { useEffect, useRef, useState } from "react";
import { useT } from "../../i18n";
import { mcp } from "../../ipc/mcp";
import { Banner, Button } from "../../ui/primitives";
import type { McpDesktop } from "./desktop";
import { errorKey } from "./model";

/** Plaintext stays in this mounted panel only; copy does not populate React state or the DOM. */
export function AccessPanel({ desktop }: { desktop: McpDesktop }) {
  const t = useT();
  const [value, setValue] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const alive = useRef(true);
  const generation = useRef(0);
  const locked = useRef(false);
  useEffect(() => {
    alive.current = true;
    const hide = () => { generation.current++; setValue(null); };
    document.addEventListener("visibilitychange", hide);
    return () => { alive.current = false; generation.current++; document.removeEventListener("visibilitychange", hide); };
  }, []);
  const run = async (kind: "show" | "copy" | "regenerate") => {
    if (locked.current) return;
    locked.current = true;
    const request = ++generation.current;
    setBusy(true); setError(null); setMessage(null);
    if (kind === "regenerate") setValue(null);
    try {
      const token = kind === "regenerate" ? await mcp.regenerateToken({ reveal: false }) : await mcp.token({ reveal: true });
      if (!alive.current || request !== generation.current) return;
      if (kind === "regenerate") { setMessage("mcp-token-regenerated"); return; }
      if (!token.value) throw new Error("Missing token");
      if (kind === "show") setValue(token.value);
      else { await desktop.copy(token.value); if (alive.current) setMessage("mcp-copied"); }
    } catch (e) { if (alive.current) setError(errorKey(e)); }
    finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  return <section className="os-mcp-stack" aria-label={t("mcp-access")} aria-busy={busy}>
    <p>{t("mcp-token-help")}</p>
    <code className="os-mcp-token" aria-label={t("mcp-token")}>{value ?? "••••••••"}</code>
    <div className="os-mcp-actions">
      <Button aria-disabled={busy} onPress={() => {
        if (locked.current) return;
        if (value !== null) { generation.current++; setValue(null); }
        else void run("show");
      }}>{t(value === null ? "mcp-show-token" : "mcp-hide-token")}</Button>
      <Button aria-disabled={busy} onPress={() => void run("copy")}>{t("mcp-copy-token")}</Button>
      <Button aria-disabled={busy} onPress={() => void run("regenerate")}>{t("mcp-regenerate-token")}</Button>
    </div>
    {message && <Banner>{t(message)}</Banner>}
    {error && <Banner kind="error">{t(error)}</Banner>}
  </section>;
}
