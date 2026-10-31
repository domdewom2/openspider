import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { mcp } from "../../ipc/mcp";
import type { ToolInfo } from "../../ipc/gen/ToolInfo";
import { Button, Checkbox } from "../../ui/primitives";
import { MCP_CLASSES } from "./model";
import type { McpController } from "./useMcpController";

export function ToolsPanel({ controller, download }: { controller: McpController; download: () => void }) {
  const t = useT();
  const [tools, setTools] = useState<ToolInfo[] | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [reload, setReload] = useState(0);
  const { prefs, revision, busy, available, perform, refreshSettings, report } = controller;
  const classes = [...new Set(["read", ...(prefs?.classes ?? [])])];
  useEffect(() => {
    let live = true;
    setLoadFailed(false);
    void mcp.tools().then(value => { if (live) setTools(value); }).catch(e => { if (live) { setLoadFailed(true); report(e); } });
    return () => { live = false; };
  }, [revision, reload, report]);
  return <section className="os-mcp-stack" aria-label={t("mcp-tools")}>
    <fieldset className="os-mcp-classes"><legend>{t("mcp-classes")}</legend>
      {MCP_CLASSES.map(kind => <Checkbox key={kind} isSelected={kind === "read" || classes.includes(kind)} isDisabled={kind === "read" || busy || !available}
        onChange={enabled => void perform(async () => {
          const next = new Set(classes); next.add("read");
          if (enabled) next.add(kind); else next.delete(kind);
          await mcp.setClasses({ classes: MCP_CLASSES.filter(c => next.has(c)) });
          await refreshSettings();
        })}>{t(`mcp-class-${kind}`)}</Checkbox>)}
    </fieldset>
    <p>{t("mcp-classes-help")}</p>
    {loadFailed && tools !== null && <p role="status">{t("mcp-load-failed")}</p>}
    {tools === null ? <p role="status">{t(loadFailed ? "mcp-load-failed" : "mcp-loading")}</p> : tools.length === 0 ? <p>{t("n03-mcp-no-tools")}</p> :
      <div className="os-mcp-scroll"><table className="os-mcp-table"><caption>{t("mcp-tools")}</caption>
        <thead><tr><th>{t("mcp-enabled")}</th><th>{t("mcp-tool")}</th><th>{t("mcp-class")}</th><th>{t("mcp-description")}</th><th>{t("mcp-schema")}</th></tr></thead>
        <tbody>{tools.map(tool => <tr key={tool.name}>
          <td><Checkbox isSelected={tool.enabled} isDisabled={busy || !available || !classes.includes(tool.class)} onChange={enabled => void perform(async () => {
            await mcp.setToolEnabled({ name: tool.name, enabled });
          })}>{tool.name}</Checkbox></td>
          <td><code>{tool.name}</code></td><td>{tool.class}</td><td>{tool.description}</td>
          <td><details><summary>{t("mcp-schema")}</summary><pre>{JSON.stringify(tool.schema, null, 2)}</pre></details></td>
        </tr>)}</tbody>
      </table></div>}
    <div className="os-mcp-actions">
      <Button isDisabled={busy} onPress={() => setReload(v => v + 1)}>{t("mcp-refresh")}</Button>
      <Button isDisabled={busy || !available || tools === null} onPress={download}>{t("mcp-markdown")}</Button>
    </div>
  </section>;
}
