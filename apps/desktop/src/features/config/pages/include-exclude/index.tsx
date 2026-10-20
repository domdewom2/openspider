// Einschließen/Ausschließen (CRAWL-109/110) mit Regex-Tester (CRAWL-111): Der Tester kodiert die Test-URL wie die Engine, gleicht
// mit derselben Semantik ab (Vollübereinstimmung oder Teilstück laut Schalter) und markiert Syntaxfehler zeilengenau.
import { useEffect, useState } from "react";
import { useT } from "../../../../i18n";
import { Banner, TextField } from "../../../../ui/primitives";
import { messageText, useConfig } from "../../frame/context";
import { definePage, f } from "../../frame/definePage";
import { useDebouncedValue } from "../../frame/hooks";
import { isTestError, testIncludeExclude, type IncludeExcludeResult, type LineRef, type TestError } from "../../frame/ruleTester";
import type { FieldDef } from "../../frame/types";

const fields: FieldDef[] = [
  f("scope.include", { kind: "lines", sectionKey: "config-sec-include", hintKey: "config-hint-include" }),
  f("scope.exclude", { kind: "lines", sectionKey: "config-sec-exclude", hintKey: "config-hint-exclude" }),
  f("scope.regex_partial_match", { sectionKey: "config-sec-match", hintKey: "config-hint-partial" }),
];

const listLabel = (t: ReturnType<typeof useT>, r: LineRef): string => t("config-ie-line", { list: t(r.list === "include" ? "config-ie-list-include" : "config-ie-list-exclude"), line: r.line });

export function IncludeExcludeTester() {
  const t = useT();
  const { draft } = useConfig();
  const [input, setInput] = useState("");
  const include = draft.get<string[] | undefined>("scope.include") ?? [];
  const exclude = draft.get<string[] | undefined>("scope.exclude") ?? [];
  const partial = draft.get<boolean | undefined>("scope.regex_partial_match") ?? false;
  const watched = useDebouncedValue({ input, include, exclude, partial }, 150);
  const [result, setResult] = useState<IncludeExcludeResult | TestError | null>(null);
  useEffect(() => {
    let live = true;
    if (watched.input.trim() === "") {
      setResult(null);
      return;
    }
    void testIncludeExclude(watched).then((r) => live && setResult(r));
    return () => {
      live = false;
    };
  }, [watched]);
  return (
    <div className="os-cfg-tester" data-testid="regex-tester">
      <h3 className="os-cfg-section">{t("config-ie-tester")}</h3>
      <TextField label={t("config-test-url")} value={input} onChange={setInput} placeholder="https://www.example.test/seite" className="os-cfg-input os-cfg-wide" />
      {result && isTestError(result) && <Banner kind="error">{messageText(t, result.error.messageKey, result.error.args, result.error.detail)}</Banner>}
      {result && !isTestError(result) && (
        <div data-testid="regex-result" data-verdict={result.verdict}>
          <p>
            <strong>{t(`config-ie-verdict-${result.verdict}`)}</strong>
            {result.match && <span data-testid="regex-match"> · {listLabel(t, result.match)}</span>}
          </p>
          <p className="os-hint">
            {t("config-ie-encoded")}: <code>{result.encoded}</code>
          </p>
          {result.partialOnly.map((r) => (
            <p key={`${r.list}${r.line}`} className="os-cfg-warning" data-testid="regex-partial-hint">
              {t("config-ie-partial", { line: listLabel(t, r) })}
            </p>
          ))}
          {result.invalid.map((r) => (
            <p key={`${r.list}${r.line}`} className="os-error" data-testid="regex-invalid">
              {t("config-ie-invalid", { line: listLabel(t, r) })}
            </p>
          ))}
        </div>
      )}
    </div>
  );
}

export const page = definePage({
  id: "include-exclude",
  order: 90,
  fields,
  anchors: ["scope.include", "scope.exclude"],
  descriptionKey: "config-ie-desc",
  After: IncludeExcludeTester,
});
