// URL-Umschreibung (CRAWL-103/104) mit Testfeld (CRAWL-107): Das Ergebnis aller aktiven Umschreibungen erscheint bei jeder
// Änderung einer Regel sofort, ohne Speichern – berechnet vom Kern mit derselben Funktion wie in der Engine.
import { useEffect, useState } from "react";
import { ArrowDown, ArrowUp, Trash2 } from "lucide-react";
import { useT } from "../../../../i18n";
import { Banner, Button, IconButton, TextField } from "../../../../ui/primitives";
import { issueText, messageText, useConfig, useConfigValue } from "../../frame/context";
import { definePage, f } from "../../frame/definePage";
import { FieldFrame } from "../../frame/fields";
import { useDebouncedValue } from "../../frame/hooks";
import { issuesFor } from "../../frame/model";
import { isTestError, testRewrite, type RewriteResult, type TestError } from "../../frame/ruleTester";
import type { FieldDef } from "../../frame/types";

interface Row {
  pattern: string;
  replacement: string;
}

const fields: FieldDef[] = [
  f("rewrite.remove_parameters", { kind: "lines", sectionKey: "config-sec-rewrite-params", hintKey: "config-hint-remove-params" }),
  f("rewrite.remove_all_parameters"),
  f("rewrite.regex_replace", { hidden: true, labelKey: "config-rewrite-regex" }),
];

function RegexTable() {
  const t = useT();
  const { draft } = useConfig();
  const { value, set, lock } = useConfigValue<Row[] | undefined>("rewrite.regex_replace");
  const rows = value ?? [];
  const update = (i: number, patch: Partial<Row>) => set(rows.map((r, n) => (n === i ? { ...r, ...patch } : r)));
  const move = (i: number, d: -1 | 1) => {
    const j = i + d;
    if (j < 0 || j >= rows.length) return;
    const next = [...rows];
    [next[i], next[j]] = [next[j] as Row, next[i] as Row];
    set(next);
  };
  return (
    <FieldFrame path="rewrite.regex_replace" showIssues={false}>
      <h3 className="os-cfg-section">{t("config-rewrite-regex")}</h3>
      <p className="os-hint">{t("config-rewrite-regex-hint")}</p>
      <table className="os-cfg-table" data-testid="rewrite-rules">
        <thead>
          <tr>
            <th scope="col">#</th>
            <th scope="col">{t("config-rewrite-pattern")}</th>
            <th scope="col">{t("config-rewrite-replacement")}</th>
            <th scope="col">
              <span className="os-sr-only">{t("config-rewrite-actions")}</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => {
            const errs = issuesFor(draft.issues, `rewrite.regex_replace[${i}]`).filter((x) => !x.warning);
            return (
              <tr key={i} data-row={i + 1} data-invalid={errs.length > 0 ? "true" : undefined}>
                <th scope="row">{i + 1}</th>
                <td>
                  <TextField label={t("config-rewrite-pattern-n", { n: i + 1 })} value={r.pattern} onChange={(pattern) => update(i, { pattern })} isDisabled={lock !== null} error={errs[0] ? issueText(t, errs[0]) : null} className="os-cfg-input os-cfg-wide" />
                </td>
                <td>
                  <TextField label={t("config-rewrite-replacement-n", { n: i + 1 })} value={r.replacement} onChange={(replacement) => update(i, { replacement })} isDisabled={lock !== null} className="os-cfg-input os-cfg-wide" />
                </td>
                <td className="os-cfg-actions">
                  <IconButton label={t("config-rewrite-up", { n: i + 1 })} icon={<ArrowUp size={14} />} onPress={() => move(i, -1)} isDisabled={lock !== null || i === 0} />
                  <IconButton label={t("config-rewrite-down", { n: i + 1 })} icon={<ArrowDown size={14} />} onPress={() => move(i, 1)} isDisabled={lock !== null || i === rows.length - 1} />
                  <IconButton label={t("config-rewrite-remove", { n: i + 1 })} icon={<Trash2 size={14} />} onPress={() => set(rows.filter((_, n) => n !== i))} isDisabled={lock !== null} />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      <Button variant="secondary" onPress={() => set([...rows, { pattern: "", replacement: "" }])} isDisabled={lock !== null}>
        {t("config-rewrite-add")}
      </Button>
    </FieldFrame>
  );
}

function RewriteTester() {
  const t = useT();
  const { draft } = useConfig();
  const [input, setInput] = useState("");
  const params = draft.get<string[] | undefined>("rewrite.remove_parameters") ?? [];
  const all = draft.get<boolean | undefined>("rewrite.remove_all_parameters") ?? false;
  const rules = draft.get<Row[] | undefined>("rewrite.regex_replace") ?? [];
  const watched = useDebouncedValue({ input, params, all, rules }, 150);
  const [result, setResult] = useState<RewriteResult | TestError | null>(null);
  useEffect(() => {
    let live = true;
    if (watched.input.trim() === "") {
      setResult(null);
      return;
    }
    void testRewrite({ removeParameters: watched.params, removeAllParameters: watched.all, rules: watched.rules, input: watched.input }).then((r) => live && setResult(r));
    return () => {
      live = false;
    };
  }, [watched]);
  return (
    <div className="os-cfg-tester" data-testid="rewrite-tester">
      <h3 className="os-cfg-section">{t("config-rewrite-tester")}</h3>
      <TextField label={t("config-test-url")} value={input} onChange={setInput} placeholder="https://www.example.test/seite?utm_source=x" className="os-cfg-input os-cfg-wide" />
      {result && isTestError(result) && <Banner kind="error">{messageText(t, result.error.messageKey, result.error.args, result.error.detail)}</Banner>}
      {result && !isTestError(result) && (
        <div data-testid="rewrite-result">
          <p>
            {t("config-rewrite-result")}: <code data-testid="rewrite-output">{result.output}</code>
          </p>
          <ol className="os-cfg-steps" aria-label={t("config-rewrite-steps")}>
            {result.stages.map((s, i) => (
              <li key={i} data-matched={s.matched ? "true" : "false"}>
                <span>{s.kind === "parameters" ? t("config-rewrite-step-params") : t("config-rewrite-step-regex", { line: s.line ?? 0 })}</span>
                {s.error ? <span className="os-error"> {s.error}</span> : <code>{s.after}</code>}
              </li>
            ))}
          </ol>
        </div>
      )}
    </div>
  );
}

export const page = definePage({
  id: "rewrite",
  order: 80,
  fields,
  anchors: ["rewrite"],
  descriptionKey: "config-rewrite-desc",
  After: () => (
    <>
      <RegexTable />
      <RewriteTester />
    </>
  ),
});
