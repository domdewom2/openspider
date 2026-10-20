// HTTP-Header (CRAWL-115/116): eigene Request-Header als Tabelle (Name, Wert, optional Host) und Accept-Language.
import { Trash2 } from "lucide-react";
import { useT } from "../../../../i18n";
import { Button, IconButton, Select, TextField } from "../../../../ui/primitives";
import { issueText, useConfig, useConfigValue } from "../../frame/context";
import { definePage, f } from "../../frame/definePage";
import { FieldFrame, fieldLabelKey } from "../../frame/fields";
import { issuesFor } from "../../frame/model";
import type { FieldDef } from "../../frame/types";

interface Header {
  name: string;
  value: string;
  host: string | null;
}

/** Gängige Sprach-Tags für `Accept-Language` (CRAWL-116); der Wert bleibt frei editierbar. */
export const ACCEPT_LANGUAGE_PRESETS: readonly string[] = ["de-DE,de;q=0.9", "de-DE,de;q=0.9,en;q=0.8", "en-US,en;q=0.9", "en-GB,en;q=0.9", "fr-FR,fr;q=0.9", "es-ES,es;q=0.9", "it-IT,it;q=0.9", "nl-NL,nl;q=0.9"];

const fields: FieldDef[] = [f("http.headers", { hidden: true, labelKey: "config-headers-table" }), f("http.accept_language", { hidden: true })];

function HeaderTable() {
  const t = useT();
  const { draft } = useConfig();
  const { value, set, lock } = useConfigValue<Header[] | undefined>("http.headers");
  const rows = value ?? [];
  const update = (i: number, patch: Partial<Header>) => set(rows.map((r, n) => (n === i ? { ...r, ...patch } : r)));
  return (
    <FieldFrame path="http.headers" showIssues={false}>
      <h3 className="os-cfg-section">{t("config-headers-table")}</h3>
      <p className="os-hint">{t("config-headers-hint")}</p>
      <table className="os-cfg-table" data-testid="headers-table">
        <thead>
          <tr>
            <th scope="col">{t("config-headers-name")}</th>
            <th scope="col">{t("config-headers-value")}</th>
            <th scope="col">{t("config-headers-host")}</th>
            <th scope="col">
              <span className="os-sr-only">{t("config-rewrite-actions")}</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => {
            const errs = issuesFor(draft.issues, `http.headers[${i}]`).filter((x) => !x.warning);
            return (
              <tr key={i} data-row={i + 1} data-invalid={errs.length > 0 ? "true" : undefined}>
                <td>
                  <TextField label={t("config-headers-name-n", { n: i + 1 })} value={r.name} onChange={(name) => update(i, { name })} isDisabled={lock !== null} error={errs[0] ? issueText(t, errs[0]) : null} className="os-cfg-input" />
                </td>
                <td>
                  <TextField label={t("config-headers-value-n", { n: i + 1 })} value={r.value} onChange={(value) => update(i, { value })} isDisabled={lock !== null} className="os-cfg-input os-cfg-wide" />
                </td>
                <td>
                  <TextField label={t("config-headers-host-n", { n: i + 1 })} value={r.host ?? ""} onChange={(host) => update(i, { host: host.trim() === "" ? null : host })} isDisabled={lock !== null} placeholder={t("config-headers-all-hosts")} className="os-cfg-input" />
                </td>
                <td className="os-cfg-actions">
                  <IconButton label={t("config-headers-remove", { n: i + 1 })} icon={<Trash2 size={14} />} onPress={() => set(rows.filter((_, n) => n !== i))} isDisabled={lock !== null} />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      <Button variant="secondary" onPress={() => set([...rows, { name: "", value: "", host: null }])} isDisabled={lock !== null}>
        {t("config-headers-add")}
      </Button>
    </FieldFrame>
  );
}

function AcceptLanguage() {
  const t = useT();
  const { value, set, lock } = useConfigValue<string | null | undefined>("http.accept_language");
  const options = [{ value: "", label: t("config-al-pick") }, ...ACCEPT_LANGUAGE_PRESETS.map((p) => ({ value: p, label: p }))];
  return (
    <FieldFrame path="http.accept_language">
      <h3 className="os-cfg-section">{t("config-al-title")}</h3>
      <div className="os-cfg-inline">
        <TextField
          label={t(fieldLabelKey("http.accept_language"))}
          value={value ?? ""}
          onChange={(v) => set(v.trim() === "" ? null : v)}
          isDisabled={lock !== null}
          description={t("config-al-hint")}
          placeholder="de-DE,de;q=0.9"
          className="os-cfg-input os-cfg-wide"
        />
        <Select label={t("config-al-examples")} value="" onChange={(v) => v !== "" && set(v)} options={options} isDisabled={lock !== null} />
      </div>
    </FieldFrame>
  );
}

export const page = definePage({
  id: "headers",
  order: 97,
  fields,
  anchors: ["http.headers", "http.accept_language"],
  descriptionKey: "config-headers-desc",
  After: () => (
    <>
      <HeaderTable />
      <AcceptLanguage />
    </>
  ),
});
