import { useState } from "react";
import { useT } from "../../../../i18n";
import { settings as api } from "../../../../ipc/settings";
import type { CertificateChainDto } from "../../../../ipc/gen/CertificateChainDto";
import { usePrefs } from "../../../../state/prefs";
import { Button, Select, TextField } from "../../../../ui/primitives";
import { RestartNote } from "../../frame/RestartNote";
import { sectionTitleKey, type SettingsSection } from "../../frame/types";
import { SettingInput, SettingsError, useSettingsAction } from "../storage/shared";
import { certificateIssuer } from "./certificates";

export function Network() {
  const t = useT();
  const network = usePrefs((s) => s.dto?.network);
  const [draft, setDraft] = useState(network?.proxy);
  const [bypass, setBypass] = useState(network?.proxy.bypass.join(", ") ?? "");
  const [seen, setSeen] = useState(network?.proxy);
  if (seen !== network?.proxy) { setSeen(network?.proxy); setDraft(network?.proxy); setBypass(network?.proxy.bypass.join(", ") ?? ""); }
  const [password, setPassword] = useState("");
  const [url, setUrl] = useState("");
  const [chain, setChain] = useState<CertificateChainDto | null>(null);
  const [probeUrl, setProbeUrl] = useState("");
  const [passwordSaved, setPasswordSaved] = useState(false);
  const action = useSettingsAction();
  if (!network || !draft) return null;
  const valid = draft.mode !== "manual" || (draft.host.trim() !== "" && !/[\s/@]/.test(draft.host) && Number.isInteger(draft.port) && draft.port >= 1 && draft.port <= 65535);
  const changeMode = (mode: string) => {
    setDraft({ ...draft, mode });
    if (mode !== "manual") void action.run(() => action.save({ network: { proxy: { mode } } }));
  };
  return <div className="os-settings-group">
    <Select label={t("u14-proxy-mode")} value={draft.mode} anchor="network.proxy" isDisabled={action.busy}
      options={["system", "none", "manual"].map((value) => ({ value, label: t(`u14-proxy-${value}`) }))} onChange={changeMode} />
    {draft.mode === "manual" && <fieldset className="os-settings-group" disabled={action.busy}>
      <legend>{t("u14-proxy-manual")}</legend>
      <Select label={t("u14-proxy-type")} value={draft.kind} options={["http", "https", "socks5"].map((value) => ({ value, label: value.toUpperCase() }))}
        onChange={(kind) => setDraft({ ...draft, kind })} />
      <TextField label={t("u14-proxy-host")} value={draft.host} onChange={(host) => setDraft({ ...draft, host })} />
      <TextField label={t("u14-proxy-port")} type="number" value={String(draft.port)} onChange={(port) => setDraft({ ...draft, port: Number(port) })} />
      <TextField label={t("u14-proxy-username")} value={draft.username ?? ""} onChange={(username) => setDraft({ ...draft, username: username || null })} />
      <TextField label={t("u14-proxy-bypass")} value={bypass} description={t("u14-proxy-bypass-hint")}
        onChange={setBypass} />
      {!valid && <p className="os-error" role="alert">{t("u14-proxy-invalid")}</p>}
      <Button isDisabled={!valid || action.busy} onPress={() => void action.run(() => action.save({ network: { proxy: { ...draft, bypass: bypass.split(",").map((s) => s.trim()).filter(Boolean) } } }))}>{t("u14-proxy-save")}</Button>
      <TextField label={t("u14-proxy-password")} type="password" value={password} onChange={(v) => { setPassword(v); setPasswordSaved(false); }} />
      <Button isDisabled={!password || action.busy} onPress={() => void action.run(async () => { await api.setProxyPassword({ password }); setPassword(""); setPasswordSaved(true); })}>{t("u14-password-save")}</Button>
      <Button isDisabled={action.busy} onPress={() => void action.run(async () => { await api.setProxyPassword({ password: null }); setPassword(""); setPasswordSaved(true); })}>{t("u14-password-clear")}</Button>
      {passwordSaved && <p role="status">{t("u14-password-saved")}</p>}
    </fieldset>}
    <p className="os-settings-note">{t("u14-proxy-scope")}</p>
    <RestartNote settingPath="network.proxy" />
    <h3>{t("u14-certificates")}</h3><p>{t("u14-certificates-os")}</p>
    <ul>{network.trustedCertificates.map((path) => <li key={path}>{path} <Button isDisabled={action.busy}
      onPress={() => void action.run(() => action.save({ network: { trustedCertificates: network.trustedCertificates.filter((p) => p !== path) } }))}>{t("u14-remove", { name: path })}</Button></li>)}</ul>
    <SettingInput label={t("u14-certificate-file")} value="" path="network.trustedCertificates" chooser="certificate" disabled={action.busy}
      validate={(v) => /\.(crt|pem|cer|der)$/i.test(v) ? null : t("u14-certificate-invalid")}
      commit={async (path) => { await action.save({ network: { trustedCertificates: [...new Set([...network.trustedCertificates, path])] } }); }} />
    <TextField label={t("u14-certificate-url")} type="url" value={url} onChange={(value) => { setUrl(value); setChain(null); }} />
    <Button isDisabled={action.busy || !/^https:\/\/[^\s/]+/i.test(url)} onPress={() => void action.run(async () => {
      const result = await api.probeCertificates({ url }); setChain(result); setProbeUrl(url);
    })}>{t("u14-certificate-probe")}</Button>
    {chain && <div role="region" aria-label={t("u14-certificate-chain")}>
      <p>{probeUrl}</p><p>{t(chain.trusted ? "u14-certificate-trusted" : "u14-certificate-untrusted")}</p>
      {chain.certificates.map((pem, i) => { let issuer = ""; try { issuer = certificateIssuer(pem); } catch { /* raw chain remains inspectable */ }
        return <details key={i}><summary>{t("u14-certificate-issuer", { index: i + 1, issuer: issuer || t("u14-certificate-unknown") })}</summary><pre style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{pem}</pre></details>;
      })}
      <Button isDisabled={action.busy || chain.certificates.length === 0} onPress={() => void action.run(async () => {
        const paths = await api.importCertificates({ certificates: chain.certificates });
        await action.save({ network: { trustedCertificates: [...network.trustedCertificates, ...paths] } }); setChain(null);
      })}>{t("u14-certificate-add")}</Button>
    </div>}
    <RestartNote settingPath="network.trusted_certificates" /><SettingsError error={action.error} />
  </div>;
}
export const section: SettingsSection = { id: "network", order: 60, titleKey: sectionTitleKey("network"), Component: Network };
