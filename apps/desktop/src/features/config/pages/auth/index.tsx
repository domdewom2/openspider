// Authentifizierung (CRAWL-117 … 121): Standard-Anmeldung (Basic; Digest erkennt der Kern aus der Challenge), formularbasierte
// Anmeldungen (Tabelle), Sitzungsschutz-Muster und Verweise auf Profile. Passwörter stehen nie in der Konfiguration oder im
// Entwurf: Sie gehen nur beim Speichern als Argument an `auth.saveProfile` (Schlüsselbund, OUT-223/OUT-225).
import { useState } from "react";
import { Trash2 } from "lucide-react";
import { useT } from "../../../../i18n";
import { auth } from "../../../../ipc/auth";
import { toCmdError } from "../../../../ipc/errors";
import { Button, IconButton, TextField } from "../../../../ui/primitives";
import { issueText, messageText, useConfig, useConfigValue } from "../../frame/context";
import { definePage, f } from "../../frame/definePage";
import { FieldFrame } from "../../frame/fields";
import { issuesFor } from "../../frame/model";
import type { FieldDef } from "../../frame/types";

interface BasicAuth {
  prefix: string;
  username: string;
  secret_ref: string;
}

interface FormLogin {
  login_url: string;
  profile: string | null;
  captured_at: string | null;
  cookie_count: number;
}

const fields: FieldDef[] = [
  f("auth.basic", { hidden: true, labelKey: "config-auth-basic" }),
  f("auth.form", { hidden: true, labelKey: "config-auth-form" }),
  f("auth.session_guard", { kind: "lines", sectionKey: "config-sec-auth-guard", hintKey: "config-hint-session-guard" }),
  f("auth.profile_refs", { kind: "lines", sectionKey: "config-sec-auth-profiles", hintKey: "config-hint-profile-refs" }),
];

/** Name des Schlüsselbund-Eintrags zu einer Zeile: stabil aus dem URL-Präfix, damit ein Eintrag nicht doppelt entsteht. */
export function secretRefFor(prefix: string, index: number): string {
  const slug = prefix.toLowerCase().replace(/^[a-z][a-z0-9+.-]*:\/\//, "").replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
  return `basic-${slug || index + 1}`;
}

/** Host-Teil eines URL-Präfixes für das Profil (`https://stage.example.test/app/` → `stage.example.test`). */
export function hostOfPrefix(prefix: string): string {
  return prefix.replace(/^[a-z][a-z0-9+.-]*:\/\//i, "").replace(/[/?#].*$/, "");
}

function BasicTable() {
  const t = useT();
  const { draft } = useConfig();
  const { value, set, lock } = useConfigValue<BasicAuth[] | undefined>("auth.basic");
  const rows = value ?? [];
  const [passwords, setPasswords] = useState<Record<number, string>>({});
  const [status, setStatus] = useState<Record<number, { kind: "saved" | "error"; text: string }>>({});
  const update = (i: number, patch: Partial<BasicAuth>) => set(rows.map((r, n) => (n === i ? { ...r, ...patch } : r)));

  const savePassword = async (i: number) => {
    const row = rows[i];
    const secret = passwords[i];
    if (!row || !secret) return;
    const name = row.secret_ref || secretRefFor(row.prefix, i);
    try {
      await auth.saveProfile({ profile: { name, kind: "basic", host: hostOfPrefix(row.prefix), username: row.username, secret } });
      if (row.secret_ref !== name) update(i, { secret_ref: name });
      setPasswords((p) => ({ ...p, [i]: "" }));
      setStatus((s) => ({ ...s, [i]: { kind: "saved", text: t("config-auth-saved") } }));
    } catch (e) {
      const err = toCmdError(e);
      setStatus((s) => ({ ...s, [i]: { kind: "error", text: messageText(t, err.messageKey, err.args, err.detail) } }));
    }
  };

  return (
    <FieldFrame path="auth.basic" showIssues={false}>
      <h3 className="os-cfg-section">{t("config-auth-basic")}</h3>
      <p className="os-hint">{t("config-auth-basic-hint")}</p>
      <table className="os-cfg-table" data-testid="auth-basic">
        <thead>
          <tr>
            <th scope="col">{t("config-auth-prefix")}</th>
            <th scope="col">{t("config-auth-user")}</th>
            <th scope="col">{t("config-auth-password")}</th>
            <th scope="col">
              <span className="os-sr-only">{t("config-rewrite-actions")}</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => {
            const errs = issuesFor(draft.issues, `auth.basic[${i}]`).filter((x) => !x.warning);
            const st = status[i];
            return (
              <tr key={i} data-row={i + 1}>
                <td>
                  <TextField label={t("config-auth-prefix-n", { n: i + 1 })} value={r.prefix} onChange={(prefix) => update(i, { prefix })} isDisabled={lock !== null} error={errs[0] ? issueText(t, errs[0]) : null} placeholder="https://stage.example.test/" className="os-cfg-input os-cfg-wide" />
                </td>
                <td>
                  <TextField label={t("config-auth-user-n", { n: i + 1 })} value={r.username} onChange={(username) => update(i, { username })} isDisabled={lock !== null} className="os-cfg-input" />
                </td>
                <td>
                  <TextField
                    label={t("config-auth-password-n", { n: i + 1 })}
                    type="password"
                    value={passwords[i] ?? ""}
                    onChange={(v) => setPasswords((p) => ({ ...p, [i]: v }))}
                    isDisabled={lock !== null}
                    placeholder={r.secret_ref ? t("config-auth-password-stored") : ""}
                    className="os-cfg-input"
                  />
                  <Button variant="secondary" onPress={() => void savePassword(i)} isDisabled={lock !== null || !(passwords[i] ?? "").length || r.prefix.trim() === ""}>
                    {t("config-auth-save-password")}
                  </Button>
                  {st && (
                    <p className={st.kind === "error" ? "os-error" : "os-hint"} role={st.kind === "error" ? "alert" : "status"}>
                      {st.text}
                    </p>
                  )}
                </td>
                <td className="os-cfg-actions">
                  <IconButton label={t("config-auth-remove", { n: i + 1 })} icon={<Trash2 size={14} />} onPress={() => set(rows.filter((_, n) => n !== i))} isDisabled={lock !== null} />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      <Button variant="secondary" onPress={() => set([...rows, { prefix: "", username: "", secret_ref: "" }])} isDisabled={lock !== null}>
        {t("config-auth-add")}
      </Button>
    </FieldFrame>
  );
}

function FormLogins() {
  const t = useT();
  const { value, set, lock } = useConfigValue<FormLogin[] | undefined>("auth.form");
  const rows = value ?? [];
  return (
    <FieldFrame path="auth.form">
      <h3 className="os-cfg-section">{t("config-auth-form")}</h3>
      <p className="os-hint">{t("config-auth-form-hint")}</p>
      {rows.length === 0 ? (
        <p className="os-hint">{t("config-auth-form-none")}</p>
      ) : (
        <table className="os-cfg-table" data-testid="auth-form">
          <thead>
            <tr>
              <th scope="col">{t("config-auth-login-url")}</th>
              <th scope="col">{t("config-auth-status")}</th>
              <th scope="col">{t("config-auth-captured")}</th>
              <th scope="col">{t("config-auth-cookies")}</th>
              <th scope="col">
                <span className="os-sr-only">{t("config-rewrite-actions")}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr key={i}>
                <td>{r.login_url}</td>
                <td>{r.cookie_count > 0 ? t("config-auth-status-active") : t("config-auth-status-empty")}</td>
                <td>{r.captured_at ?? "–"}</td>
                <td>{r.cookie_count}</td>
                <td className="os-cfg-actions">
                  <IconButton label={t("config-auth-remove-form", { url: r.login_url })} icon={<Trash2 size={14} />} onPress={() => set(rows.filter((_, n) => n !== i))} isDisabled={lock !== null} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </FieldFrame>
  );
}

export const page = definePage({
  id: "auth",
  order: 150,
  fields,
  anchors: ["auth"],
  descriptionKey: "config-auth-desc",
  Before: () => (
    <>
      <BasicTable />
      <FormLogins />
    </>
  ),
});

