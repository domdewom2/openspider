// Profile (OUT-220): Konfigurationen speichern, laden, als Standard setzen, löschen, importieren und exportieren. Geladene
// Profile landen zunächst nur im Entwurf des Dialogs; angewendet werden sie mit „OK“. Während „Läuft“ ist das Laden gesperrt.
import { useState } from "react";
import { useT } from "../../../../i18n";
import { client } from "../../../../ipc/client";
import { toCmdError } from "../../../../ipc/errors";
import { profile as profileApi } from "../../../../ipc/profile";
import { Banner, Button, Checkbox, Dialog, DialogFooter, TextField } from "../../../../ui/primitives";
import { messageText, useConfig } from "../../frame/context";
import { definePage } from "../../frame/definePage";
import { useProfileList } from "../../frame/profiles";
import { pageTitleKey } from "../../frame/types";
import { PageHeading } from "../../frame/fields";

type Pending = { kind: "overwrite"; name: string } | { kind: "delete"; name: string } | { kind: "resetDefault" } | null;

const formatDate = (iso: string): string => {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString();
};

/** Nativer Dateidialog (nur im Tauri-Fenster); sonst gibt der Nutzer den Pfad ein. */
async function pickPath(save: boolean, title: string): Promise<string | null> {
  if (client.transportName !== "tauri") return null;
  const options = save ? { title, filters: [{ name: "OpenSpider", extensions: ["osconfig"] }] } : { title, multiple: false, filters: [{ name: "OpenSpider", extensions: ["osconfig"] }] };
  return client.invoke<string | null>(save ? "dialog.save" : "dialog.open", { options });
}

function ProfilesPage() {
  const t = useT();
  const { draft, runMode, loadProfile, bumpProfiles } = useConfig();
  const list = useProfileList(true);
  const [name, setName] = useState("");
  const [makeDefault, setMakeDefault] = useState(false);
  const [pending, setPending] = useState<Pending>(null);
  const [message, setMessage] = useState<{ kind: "info" | "error"; text: string } | null>(null);
  const [importPath, setImportPath] = useState("");
  const [exportPath, setExportPath] = useState("");
  const [exportName, setExportName] = useState("");
  const readOnly = runMode === "foreign";
  const loadBlocked = runMode === "running" || runMode === "foreign";

  const fail = (e: unknown) => {
    const err = toCmdError(e);
    setMessage({ kind: "error", text: messageText(t, err.messageKey, err.args, err.detail) });
  };
  const changed = async (text: string) => {
    setMessage({ kind: "info", text });
    bumpProfiles();
    await list.reload();
  };

  const save = async (profileName: string, overwrite: boolean) => {
    const dto = draft.toDto();
    if (!dto) return;
    try {
      await profileApi.save({ name: profileName, config: dto, overwrite });
      if (makeDefault) await profileApi.setDefault({ name: profileName });
      setName("");
      await changed(t(makeDefault ? "config-profiles-saved-default" : "config-profiles-saved", { name: profileName }));
    } catch (e) {
      fail(e);
    }
  };
  const onSave = () => {
    const n = name.trim();
    if (n === "") return;
    if (list.profiles.some((p) => p.name === n)) setPending({ kind: "overwrite", name: n });
    else void save(n, false);
  };
  const setDefault = async (profileName: string) => {
    try {
      await profileApi.setDefault({ name: profileName });
      await changed(t("config-profiles-default-set", { name: profileName }));
    } catch (e) {
      fail(e);
    }
  };
  const confirm = async () => {
    const p = pending;
    setPending(null);
    if (!p) return;
    try {
      if (p.kind === "overwrite") await save(p.name, true);
      else if (p.kind === "delete") {
        await profileApi.delete({ name: p.name });
        await changed(t("config-profiles-deleted", { name: p.name }));
      } else {
        await profileApi.setDefault({ name: null });
        await changed(t("config-profiles-default-reset"));
      }
    } catch (e) {
      fail(e);
    }
  };
  const doImport = async () => {
    try {
      const info = await profileApi.import({ path: importPath.trim() });
      setImportPath("");
      await changed(t("config-profiles-imported", { name: info.name }));
    } catch (e) {
      fail(e);
    }
  };
  const doExport = async () => {
    try {
      await profileApi.export({ name: exportName, path: exportPath.trim() });
      setMessage({ kind: "info", text: t("config-profiles-exported", { name: exportName }) });
    } catch (e) {
      fail(e);
    }
  };

  const dialogTitle = !pending ? "" : pending.kind === "overwrite" ? t("config-profiles-overwrite-title") : pending.kind === "delete" ? t("config-profiles-delete-title") : t("config-profiles-reset-title");
  const dialogText = !pending
    ? ""
    : pending.kind === "overwrite"
      ? t("config-profiles-overwrite-text", { name: pending.name })
      : pending.kind === "delete"
        ? t("config-profiles-delete-text", { name: pending.name })
        : t("config-profiles-reset-text");

  return (
    <section className="os-cfg-page" aria-label={t(pageTitleKey("profiles"))} data-page-id="profiles">
      <PageHeading titleKey={pageTitleKey("profiles")} descriptionKey="config-profiles-desc" />
      {loadBlocked && (
        <Banner kind="info">
          <span data-testid="profiles-locked">{t(runMode === "running" ? "config-profile-blocked-running" : "config-profile-blocked-foreign")}</span>
        </Banner>
      )}
      {message && (
        <Banner kind={message.kind === "error" ? "error" : "info"} onDismiss={() => setMessage(null)} dismissLabel={t("config-dismiss")}>
          {message.text}
        </Banner>
      )}
      {list.error && <Banner kind="error">{messageText(t, list.error.messageKey, list.error.args, list.error.detail)}</Banner>}

      <h3 className="os-cfg-section">{t("config-profiles-list")}</h3>
      {list.profiles.length === 0 ? (
        <p className="os-hint" data-testid="profiles-empty">
          {t("config-profiles-none")}
        </p>
      ) : (
        <table className="os-cfg-table" data-testid="profiles-table">
          <thead>
            <tr>
              <th scope="col">{t("config-profiles-name")}</th>
              <th scope="col">{t("config-profiles-modified")}</th>
              <th scope="col">
                <span className="os-sr-only">{t("config-rewrite-actions")}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {list.profiles.map((p) => (
              <tr key={p.name} data-profile={p.name} data-default={p.isDefault ? "true" : undefined}>
                <th scope="row">
                  {p.name}
                  {p.isDefault && <span className="os-cfg-badge"> {t("config-profiles-default")}</span>}
                  {p.recentRank != null && <span className="os-cfg-badge" data-testid="profile-recent"> {t("config-profiles-recent", { n: p.recentRank + 1 })}</span>}
                </th>
                <td>{formatDate(p.modifiedAt)}</td>
                <td className="os-cfg-actions">
                  <Button variant="secondary" onPress={() => void loadProfile(p.name)} isDisabled={loadBlocked} aria-label={t("config-profiles-load-n", { name: p.name })}>
                    {t("config-profiles-load")}
                  </Button>
                  <Button variant="secondary" onPress={() => void setDefault(p.name)} isDisabled={readOnly || p.isDefault} aria-label={t("config-profiles-default-n", { name: p.name })}>
                    {t("config-profiles-make-default")}
                  </Button>
                  <Button variant="secondary" onPress={() => { setExportName(p.name); setExportPath(""); }} aria-label={t("config-profiles-export-n", { name: p.name })}>
                    {t("config-profiles-export")}
                  </Button>
                  <Button variant="danger" onPress={() => setPending({ kind: "delete", name: p.name })} isDisabled={readOnly} aria-label={t("config-profiles-delete-n", { name: p.name })}>
                    {t("config-profiles-delete")}
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <h3 className="os-cfg-section">{t("config-profiles-save-title")}</h3>
      <div className="os-cfg-inline">
        <TextField label={t("config-profiles-save-name")} value={name} onChange={setName} className="os-cfg-input" isDisabled={readOnly} />
        <Checkbox isSelected={makeDefault} onChange={setMakeDefault} isDisabled={readOnly}>
          {t("config-profiles-save-default")}
        </Checkbox>
        <Button variant="primary" onPress={onSave} isDisabled={readOnly || name.trim() === ""}>
          {t("config-profiles-save")}
        </Button>
      </div>
      <p className="os-hint">{t("config-profiles-save-hint")}</p>
      <Button variant="secondary" onPress={() => setPending({ kind: "resetDefault" })} isDisabled={readOnly || !list.profiles.some((p) => p.isDefault)}>
        {t("config-profiles-reset-default")}
      </Button>

      <h3 className="os-cfg-section">{t("config-profiles-import-title")}</h3>
      <div className="os-cfg-inline">
        <TextField label={t("config-profiles-import-path")} value={importPath} onChange={setImportPath} className="os-cfg-input os-cfg-wide" />
        {client.transportName === "tauri" && (
          <Button variant="secondary" onPress={() => void pickPath(false, t("config-profiles-import-title")).then((p) => p && setImportPath(p))}>
            {t("config-browse")}
          </Button>
        )}
        <Button variant="secondary" onPress={() => void doImport()} isDisabled={importPath.trim() === ""}>
          {t("config-profiles-import")}
        </Button>
      </div>

      {exportName !== "" && (
        <>
          <h3 className="os-cfg-section">{t("config-profiles-export-title", { name: exportName })}</h3>
          <div className="os-cfg-inline">
            <TextField label={t("config-profiles-export-path")} value={exportPath} onChange={setExportPath} className="os-cfg-input os-cfg-wide" />
            {client.transportName === "tauri" && (
              <Button variant="secondary" onPress={() => void pickPath(true, t("config-profiles-export-title", { name: exportName })).then((p) => p && setExportPath(p))}>
                {t("config-browse")}
              </Button>
            )}
            <Button variant="secondary" onPress={() => void doExport()} isDisabled={exportPath.trim() === ""}>
              {t("config-profiles-export")}
            </Button>
          </div>
        </>
      )}

      <Dialog
        title={dialogTitle}
        isOpen={pending !== null}
        onOpenChange={(open) => !open && setPending(null)}
        size="small"
        footer={<DialogFooter okLabel={t("config-confirm-ok")} cancelLabel={t("config-cancel")} onOk={() => void confirm()} onCancel={() => setPending(null)} okVariant={pending?.kind === "overwrite" ? "primary" : "danger"} />}
      >
        <p>{dialogText}</p>
      </Dialog>
    </section>
  );
}

export const page = definePage({ id: "profiles", order: 200, fields: [], anchors: ["profiles"], searchKeys: ["config-profiles-search"] });
page.Component = ProfilesPage;
