// User-Agent (CRAWL-114): Voreinstellung, HTTP-User-Agent und robots.txt-Token. Die Voreinstellungen stammen aus der Datendatei des Kerns.
import { useEffect, useState } from "react";
import { useT } from "../../../../i18n";
import { app } from "../../../../ipc/app";
import { Select, TextField } from "../../../../ui/primitives";
import { useConfig, useConfigValue } from "../../frame/context";
import { definePage, f } from "../../frame/definePage";
import { FieldFrame, fieldLabelKey } from "../../frame/fields";
import type { FieldDef } from "../../frame/types";
import { UA_PRESETS } from "./presets";

export const CUSTOM_PRESET = "custom";

const fields: FieldDef[] = [
  f("user_agent.preset", { kind: "text", hidden: true }),
  f("user_agent.http", { hidden: true }),
  f("user_agent.robots_token", { hidden: true }),
];

/** Wirksame Werte wie `os_fetch::user_agents::resolve`: eigene Angabe vor Voreinstellung; ohne Token gilt nur `*`. */
export function resolveUa(cfg: { preset?: string | null; http?: string | null; robots_token?: string | null }, version: string | null) {
  const preset = UA_PRESETS.find((p) => p.id === cfg.preset) ?? null;
  const http = (cfg.http ?? "").trim() || preset?.userAgent || UA_PRESETS[0]?.userAgent || "";
  const explicit = (cfg.robots_token ?? "").trim();
  const token = explicit || (preset && preset.id !== CUSTOM_PRESET ? preset.robotsToken : "*");
  const ua = version ? http.replace("{version}", version) : http;
  return { http: ua, token, missingToken: token === "*" && (cfg.http ?? "").trim() !== "" };
}

function UserAgentFields() {
  const t = useT();
  const { draft } = useConfig();
  const preset = useConfigValue<string | undefined>("user_agent.preset");
  const http = useConfigValue<string | null | undefined>("user_agent.http");
  const token = useConfigValue<string | null | undefined>("user_agent.robots_token");
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    app.info().then((i) => live && setVersion(i.version)).catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);
  const lock = preset.lock;
  const current = preset.value ?? UA_PRESETS[0]?.id ?? "openspider";
  const resolved = resolveUa({ preset: current, http: http.value, robots_token: token.value }, version);
  const selected = UA_PRESETS.find((p) => p.id === current);
  const options = UA_PRESETS.map((p) => ({ value: p.id, label: p.id === CUSTOM_PRESET ? t("config-ua-custom") : p.name }));
  if (!selected) options.push({ value: current, label: current });
  const choose = (id: string) => {
    // Eine Voreinstellung bringt UA und Token selbst mit; eigene Angaben werden dabei verworfen.
    draft.set("user_agent.preset", id);
    draft.set("user_agent.http", null);
    draft.set("user_agent.robots_token", null);
  };
  const edit = (path: "user_agent.http" | "user_agent.robots_token", value: string) => {
    if (current !== CUSTOM_PRESET) draft.set("user_agent.preset", CUSTOM_PRESET);
    draft.set(path, value === "" ? null : value);
  };
  return (
    <div className="os-cfg-fields">
      <div className="os-cfg-row">
        <FieldFrame path="user_agent.preset">
          <Select label={t(fieldLabelKey("user_agent.preset"))} value={current} onChange={choose} options={options} isDisabled={lock !== null} />
        </FieldFrame>
      </div>
      <div className="os-cfg-row">
        <FieldFrame path="user_agent.http" showIssues>
          <TextField
            label={t(fieldLabelKey("user_agent.http"))}
            value={http.value ?? (current === CUSTOM_PRESET ? "" : resolved.http)}
            onChange={(v) => edit("user_agent.http", v)}
            isDisabled={lock !== null}
            className="os-cfg-input os-cfg-wide"
          />
        </FieldFrame>
      </div>
      <div className="os-cfg-row">
        <FieldFrame path="user_agent.robots_token">
          <TextField
            label={t(fieldLabelKey("user_agent.robots_token"))}
            value={token.value ?? (current === CUSTOM_PRESET ? "" : resolved.token)}
            onChange={(v) => edit("user_agent.robots_token", v)}
            isDisabled={lock !== null}
            description={t("config-hint-robots-token")}
            className="os-cfg-input os-cfg-wide"
          />
        </FieldFrame>
      </div>
      <p className="os-hint" data-testid="ua-effective">
        {t("config-ua-effective", { ua: resolved.http, token: resolved.token })}
      </p>
      {resolved.missingToken && (
        <p className="os-cfg-warning" role="status" data-testid="ua-missing-token">
          {t("config-ua-missing-token")}
        </p>
      )}
    </div>
  );
}

export const page = definePage({
  id: "user-agent",
  order: 96,
  fields,
  anchors: ["user_agent"],
  descriptionKey: "config-ua-desc",
  searchKeys: ["config-ua-search"],
  Before: UserAgentFields,
});
