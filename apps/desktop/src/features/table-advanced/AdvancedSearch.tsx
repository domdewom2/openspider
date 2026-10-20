import { useEffect, useState, type ReactNode } from "react";
import { useT } from "../../i18n";
import { search } from "../../ipc/search";
import { toCmdError } from "../../ipc/errors";
import type { ColumnMeta } from "../../ipc/gen/ColumnMeta";
import type { RuleNode } from "../../ipc/gen/RuleNode";
import type { RuleOp } from "../../ipc/gen/RuleOp";
import type { RuleQuery } from "../../state/tabs";
import { Button } from "../../ui/primitives/Button";
import { Checkbox } from "../../ui/primitives/fields";
import { appendAt, asGroup, conditions, emptyCond, emptyGroup, formatRule, isNumericKind, isRegex, isUnary, normalizeNumber,
  operatorsFor, prune, removeAt, updateAt, type Cond, type Group, type Path } from "./query";
import { useSearchHistory } from "./stores";
import "./table-advanced.css";

type Translate = ReturnType<typeof useT>;
/** User text of a search error from the core; `err-search-timeout` is the „too expensive“ hint of UI-011. */
export function searchErrorText(t: Translate, error: unknown): string {
  const e = toCmdError(error);
  if (e.messageKey === "err-search-timeout" || e.code === "Timeout") return t("u20-search-too-expensive");
  if (e.messageKey === "err-search-syntax") return t("u20-search-syntax", { position: Number(e.args.position ?? 0) + 1 });
  if (e.messageKey === "err-search-regex") return t("u20-search-regex", { detail: e.detail ?? "" });
  return t(e.messageKey, e.args as Record<string, unknown>, { fallback: e.detail ?? e.messageKey });
}

/** Parse the text syntax in the core (`search.parse`); the UI never interprets the grammar itself. */
export async function parseQuery(text: string): Promise<RuleNode> {
  return search.parse({ text });
}

/** Checks a rule tree before it reaches the view: numbers in the UI language, regexes via the core. Errors by path. */
export async function validateRules(tree: RuleNode, columns: ColumnMeta[], locale: string, t: Translate): Promise<{ tree: RuleNode; errors: Record<string, string> }> {
  const errors: Record<string, string> = {};
  let next = tree;
  for (const { cond, path } of conditions(tree)) {
    if (isUnary(cond.op)) continue;
    const meta = columns.find(c => c.key === cond.column);
    if (meta && isNumericKind(meta.kind)) {
      const value = normalizeNumber(cond.value ?? "", locale);
      if (value === null) errors[path.join(".")] = t("u20-search-number");
      else next = updateAt(next, path, node => ({ ...node, value }) as Cond);
    } else if (isRegex(cond.op)) {
      try { await search.validateRegex({ pattern: cond.value ?? "" }); } catch (error) { errors[path.join(".")] = searchErrorText(t, error); }
    }
  }
  return { tree: next, errors };
}

export function AdvancedSearch({ columns, value, locale, onApply, onClear }: {
  columns: ColumnMeta[]; value: RuleQuery | null | undefined; locale: string;
  onApply: (rules: RuleQuery) => void; onClear: () => void;
}) {
  const t = useT();
  const history = useSearchHistory();
  const [tree, setTree] = useState<Group>(() => value ? asGroup(value.tree) : emptyGroup());
  const [caseSensitive, setCaseSensitive] = useState(value?.caseSensitive ?? false);
  const [text, setText] = useState(value?.text ?? "");
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [syntaxError, setSyntaxError] = useState<string>();
  const [busy, setBusy] = useState(false);
  useEffect(() => { void history.load().catch(() => undefined); }, []);
  useEffect(() => {
    if (!value) return;
    setTree(asGroup(value.tree)); setCaseSensitive(value.caseSensitive); setText(value.text);
  }, [value?.text, value?.caseSensitive]);
  const searchable = columns.filter(c => c.key !== "Row");
  const edit = (path: Path, update: (node: RuleNode) => RuleNode) => { setTree(old => asGroup(updateAt(old, path, update))); setErrors({}); };
  const parseText = async () => {
    try { const parsed = asGroup(await parseQuery(text)); setTree(parsed); setSyntaxError(undefined); return parsed; }
    catch (error) { setSyntaxError(searchErrorText(t, error)); return null; }
  };
  const apply = async (source: Group = tree) => {
    const pruned = prune(source);
    if (!pruned) { onClear(); return; }
    setBusy(true);
    try {
      const checked = await validateRules(pruned, columns, locale, t);
      setErrors(checked.errors);
      // Invalid input keeps the current view unchanged (UI-011 [P1]).
      if (Object.keys(checked.errors).length) return;
      const rules = { tree: checked.tree, caseSensitive, text: formatRule(checked.tree) };
      setText(rules.text); setSyntaxError(undefined);
      onApply(rules);
      // Persisting the history is debounced; the next search must not wait for it.
      void history.add(rules).catch(() => undefined);
    } finally { setBusy(false); }
  };
  const renderCond = (cond: Cond, path: Path) => {
    const meta = columns.find(c => c.key === cond.column) ?? null;
    const ops = operatorsFor(meta);
    const error = errors[path.join(".")];
    return <div className="u20-cond" role="group" aria-label={t("u20-search-condition")}>
      <label>{t("u20-search-column")}<select aria-label={t("u20-search-column")} className="os-input" value={cond.column ?? ""} onChange={e => edit(path, () => {
        const column = e.target.value || null; const next = operatorsFor(columns.find(c => c.key === column) ?? null);
        return { ...cond, column, op: next.includes(cond.op) ? cond.op : next[0] };
      })}>
        <option value="">{t("u20-search-all-columns")}</option>
        {searchable.map(c => <option key={c.key} value={c.key}>{c.key}</option>)}
      </select></label>
      <label>{t("u20-search-operator")}<select aria-label={t("u20-search-operator")} className="os-input" value={cond.op} onChange={e => edit(path, () => ({ ...cond, op: e.target.value as RuleOp }))}>
        {ops.map(op => <option key={op} value={op}>{t(`u20-op-${op}`)}</option>)}
      </select></label>
      {!isUnary(cond.op) && <label>{t("u20-search-value")}<input aria-label={t("u20-search-value")} className="os-input" value={cond.value ?? ""} aria-invalid={error ? true : undefined}
        data-invalid={error ? "true" : undefined} inputMode={meta && isNumericKind(meta.kind) ? "decimal" : undefined}
        onChange={e => edit(path, () => ({ ...cond, value: e.target.value }))}
        onKeyDown={e => { if (e.key === "Enter") { e.preventDefault(); void apply(); } }} /></label>}
      <Button onPress={() => edit([], root => removeAt(root, path))}>{t("u20-search-remove")}</Button>
      {error && <span role="alert" className="u20-error">{error}</span>}
    </div>;
  };
  const renderGroup = (group: Group, path: Path): ReactNode => <fieldset className="u20-group" data-depth={path.length}>
    <legend><label>{t("u20-search-combine")}<select aria-label={t("u20-search-combine")} className="os-input" value={group.op}
      onChange={e => edit(path, node => ({ ...node, op: e.target.value as Group["op"] }) as Group)}>
      <option value="and">{t("u20-search-and")}</option><option value="or">{t("u20-search-or")}</option>
    </select></label></legend>
    {group.children.map((child, i) => <div key={i}>{child.kind === "group" ? renderGroup(child, [...path, i]) : renderCond(child, [...path, i])}</div>)}
    <div className="u20-row">
      <Button onPress={() => edit(path, node => appendAt(node, [], emptyCond()))}>{t("u20-search-add-condition")}</Button>
      <Button onPress={() => edit(path, node => appendAt(node, [], emptyGroup()))}>{t("u20-search-add-group")}</Button>
      {path.length > 0 && <Button onPress={() => edit([], root => removeAt(root, path))}>{t("u20-search-remove-group")}</Button>}
    </div>
  </fieldset>;
  return <div className="u20-advanced" role="region" aria-label={t("u20-search-advanced")}>
    <div className="u20-row">
      <label className="u20-syntax">{t("u20-search-syntax-label")}<input aria-label={t("u20-search-syntax-label")} className="os-input" value={text} aria-invalid={syntaxError ? true : undefined}
        data-invalid={syntaxError ? "true" : undefined} onChange={e => setText(e.target.value)}
        onKeyDown={e => { if (e.key === "Enter") { e.preventDefault(); void parseText().then(parsed => parsed && apply(parsed)); } }} /></label>
      <Button onPress={() => void parseText()}>{t("u20-search-parse")}</Button>
      <label>{t("u20-search-history")}<select aria-label={t("u20-search-history")} className="os-input" value="" onChange={e => {
        const entry = history.entries[Number(e.target.value)]; if (!entry) return;
        setTree(asGroup(entry.tree)); setCaseSensitive(entry.caseSensitive); setText(entry.text); setErrors({}); setSyntaxError(undefined);
      }}>
        <option value="">{t("u20-search-history-pick", { count: history.entries.length })}</option>
        {history.entries.map((entry, i) => <option key={`${i}-${entry.text}`} value={i}>{entry.text}{entry.caseSensitive ? " (Aa)" : ""}</option>)}
      </select></label>
    </div>
    {syntaxError && <p role="alert" className="u20-error">{syntaxError}</p>}
    {renderGroup(tree, [])}
    <div className="u20-row">
      <Checkbox isSelected={caseSensitive} onChange={setCaseSensitive}>{t("u20-search-case")}</Checkbox>
      <Button variant="primary" isDisabled={busy} onPress={() => void apply()}>{t("u20-search-apply")}</Button>
      <Button onPress={() => { setTree(emptyGroup()); setText(""); setErrors({}); setSyntaxError(undefined); onClear(); }}>{t("u20-search-clear")}</Button>
    </div>
  </div>;
}
