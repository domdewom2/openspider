// „Abweichungen anzeigen“ (OUT-224): alle Werte des Entwurfs, die von den Werkseinstellungen abweichen, mit Sprung zum Feld.
import { currentLanguage, i18n, useT } from "../../../i18n";
import { Button } from "../../../ui/primitives";
import { useConfig } from "./context";
import { fieldLabelKey } from "./fields";
import { deviations } from "./model";

const show = (v: unknown): string => (typeof v === "string" ? v : JSON.stringify(v));

export function DiffPanel({ onBack }: { onBack: () => void }) {
  const t = useT();
  const { draft, goto } = useConfig();
  const rows = deviations(draft.cfg, draft.saved, draft.defaultRows);
  return (
    <section className="os-cfg-diff" aria-label={t("config-diff-title")}>
      <header className="os-cfg-page-head">
        <h2 className="os-cfg-page-title">{t("config-diff-title")}</h2>
        <Button variant="secondary" onPress={onBack}>
          {t("config-diff-back")}
        </Button>
      </header>
      {rows.length === 0 ? (
        <p className="os-hint" data-testid="config-diff-empty">
          {t("config-diff-none")}
        </p>
      ) : (
        <table className="os-cfg-table" data-testid="config-diff-table">
          <thead>
            <tr>
              <th scope="col">{t("config-diff-setting")}</th>
              <th scope="col">{t("config-diff-current")}</th>
              <th scope="col">{t("config-diff-standard")}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => {
              const key = fieldLabelKey(r.path);
              const known = i18n.has(currentLanguage(), key) || i18n.has("en", key);
              return (
                <tr key={r.path} data-path={r.path}>
                  <th scope="row">
                    <button type="button" className="os-cfg-link" onClick={() => goto(r.path)}>
                      {known ? t(key) : r.path}
                    </button>
                    {known && <code className="os-cfg-path">{r.path}</code>}
                  </th>
                  <td>
                    <code>{show(r.value)}</code>
                  </td>
                  <td>
                    <code>{show(r.standard)}</code>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </section>
  );
}
