export interface SearchFilter { name: string; contains: boolean; kind: string; expression: string; scope: string; xpath: string | null; case_sensitive: boolean }
export const SEARCH_SCOPES = ["html", "page_text", "page_text_no_anchors", "html_head", "html_body", "xpath", "content_area"] as const;
export const newFilter = (n: number): SearchFilter => ({ name: `Filter ${n}`, contains: true, kind: "text", expression: "", scope: "html", xpath: null, case_sensitive: false });
/** Single-column TXT/CSV, including quoted multiline cells and escaped quotes. */
export function importExpressions(text: string): string[] {
  const result: string[] = []; let cell = ""; let quoted = false;
  text = text.replace(/^\uFEFF/, "");
  for (let i = 0; i < text.length; i++) {
    const c = text[i]!;
    if (c === '"') { if (quoted && text[i + 1] === '"') { cell += '"'; i++; } else if (quoted || cell === "") quoted = !quoted; else cell += c; }
    else if (!quoted && (c === "\n" || c === "\r")) { result.push(cell); cell = ""; if (c === "\r" && text[i + 1] === "\n") i++; }
    else cell += c;
  }
  result.push(cell); return result;
}
export function bulkAdd(rows: SearchFilter[], expressions: string[], template: SearchFilter) {
  const next = [...rows]; const seen = new Set(rows.map(r => r.expression)); let skipped = 0; let overflow = 0;
  for (const expression of expressions) {
    if (!expression.trim() || seen.has(expression)) { skipped++; continue; }
    seen.add(expression);
    if (next.length === 100) { overflow++; continue; }
    next.push({ ...template, name: expression, expression });
  }
  return { rows: next, skipped, overflow };
}
