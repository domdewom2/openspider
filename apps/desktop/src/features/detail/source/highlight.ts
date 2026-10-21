// Leichte Syntax-Hervorhebung für HTML (UI-041): Tokens nur für den sichtbaren Bereich, ohne Abhängigkeit zu Sprachpaketen.
export interface Tok { from: number; to: number; cls: "comment" | "tag" | "attr" | "string" | "entity" | "punct" | "doctype" }
const MAX_SCAN = 400_000;

export function tokenizeHtml(source: string, base = 0): Tok[] {
  const text = source.length > MAX_SCAN ? source.slice(0, MAX_SCAN) : source;
  const out: Tok[] = []; const push = (from: number, to: number, cls: Tok["cls"]) => { if (to > from) out.push({ from: base + from, to: base + to, cls }); };
  let i = 0; const n = text.length;
  while (i < n) {
    const lt = text.indexOf("<", i); const amp = text.indexOf("&", i);
    const next = lt === -1 ? amp : amp === -1 ? lt : Math.min(lt, amp);
    if (next === -1) break;
    i = next;
    if (text[i] === "&") {
      const m = /^&(?:#\d+|#x[\da-f]+|[a-z][\da-z]*);/i.exec(text.slice(i, i + 12));
      if (m) { push(i, i + m[0].length, "entity"); i += m[0].length; } else i++;
      continue;
    }
    if (text.startsWith("<!--", i)) { const end = text.indexOf("-->", i + 4); const to = end === -1 ? n : end + 3; push(i, to, "comment"); i = to; continue; }
    if (text.startsWith("<!", i)) { const end = text.indexOf(">", i); const to = end === -1 ? n : end + 1; push(i, to, "doctype"); i = to; continue; }
    const open = /^<(\/?)([a-zA-Z][^\s/>]*)/.exec(text.slice(i, i + 80));
    if (!open) { i++; continue; }
    push(i, i + 1 + open[1]!.length, "punct"); const nameFrom = i + 1 + open[1]!.length; push(nameFrom, nameFrom + open[2]!.length, "tag");
    i = nameFrom + open[2]!.length;
    // Attribute bis zum schließenden „>“.
    while (i < n && text[i] !== ">") {
      const ch = text[i]!;
      if (/\s/.test(ch) || ch === "/") { i++; continue; }
      if (ch === "=") { push(i, i + 1, "punct"); i++; continue; }
      if (ch === '"' || ch === "'") { const end = text.indexOf(ch, i + 1); const to = end === -1 ? n : end + 1; push(i, to, "string"); i = to; continue; }
      const m = /^[^\s=/>"']+/.exec(text.slice(i, i + 200));
      if (!m) { i++; continue; }
      push(i, i + m[0].length, "attr"); i += m[0].length;
    }
    if (i < n) { push(i, i + 1, "punct"); i++; }
    // Rohtext-Elemente (script/style) bis zum passenden Endtag überspringen.
    const raw = !open[1] && /^(script|style)$/i.test(open[2]!) ? open[2]!.toLowerCase() : null;
    if (raw) { const end = text.toLowerCase().indexOf(`</${raw}`, i); i = end === -1 ? n : end; }
  }
  return out;
}
