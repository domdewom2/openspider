// Baum der strukturierten Daten und Befundzuordnung (UI-045, TECH-189).
import type { SdIssueDto } from "../../../ipc/gen/SdIssueDto";
import type { SdNode } from "../../../ipc/gen/SdNode";

export const FORMAT_ORDER = ["json-ld", "microdata", "rdfa"] as const;
export const FORMAT_LABEL: Record<string, string> = { "json-ld": "JSON-LD", microdata: "Microdata", rdfa: "RDFa" };
export type Severity = "error" | "warning" | "none";
const RANK: Record<Severity, number> = { none: 0, warning: 1, error: 2 };
export const worst = (a: Severity, b: Severity): Severity => RANK[a] >= RANK[b] ? a : b;
export const severityOf = (issue: SdIssueDto): Severity => issue.severity.toLowerCase() === "error" ? "error" : "warning";

export interface TreeEntry {
  id: string;
  kind: "format" | "item" | "property" | "parse-error";
  label: string;
  /** Wert einer Eigenschaft bzw. Fundstelle eines Parse-Fehlers. */
  value?: string;
  format: string;
  node?: SdNode;
  children: TreeEntry[];
  /** Eigene Befunde (Element bzw. Eigenschaft). */
  issues: SdIssueDto[];
  severity: Severity;
  /** Alle Befunde dieses Knotens und seiner Kinder in Baumreihenfolge. */
  all: SdIssueDto[];
}
const isParseError = (node: SdNode) => node.typePath === "Parse Error" || node.issues.some(i => /parse[_.-]?error/i.test(i.code));
/** Name der Eigenschaft eines verschachtelten Elements: letztes Pfadsegment ohne Index. */
export const propertyName = (path: string): string => path.split(".").pop()?.replace(/\[\d+\]$/, "") ?? path;
const joinPath = (parent: string, name: string) => parent ? `${parent}.${name}` : name;
const matchesProperty = (issue: SdIssueDto, itemPath: string, name: string) => issue.property === name || issue.path === joinPath(itemPath, name) || issue.path.replace(/\[\d+\]/g, "") === joinPath(itemPath, name).replace(/\[\d+\]/g, "");

function finish(entry: Omit<TreeEntry, "severity" | "all">): TreeEntry {
  const own = entry.issues.reduce<Severity>((acc, i) => worst(acc, severityOf(i)), "none");
  const severity = entry.children.reduce<Severity>((acc, c) => worst(acc, c.severity), own);
  return { ...entry, severity, all: [...entry.issues, ...entry.children.flatMap(c => c.all)] };
}
function itemEntry(node: SdNode, label: string): TreeEntry {
  if (isParseError(node)) {
    const where = node.sourceLine ? ` (${node.sourceLine}${node.sourceCol ? `:${node.sourceCol}` : ""})` : "";
    return finish({ id: `item:${node.id}`, kind: "parse-error", label: `Parse Error${where}`, format: node.format, node, children: [], issues: node.issues });
  }
  const children: TreeEntry[] = [];
  const nested = [...node.children]; const used = new Set<SdNode>();
  const order = node.propertyOrder.length > 0 ? node.propertyOrder : node.props.map(p => p.name);
  const leaf = (name: string, value: unknown, index: number): TreeEntry => finish({ id: `prop:${node.id}:${name}:${index}`, kind: "property", label: name,
    value: value === null || value === undefined ? "" : typeof value === "object" ? JSON.stringify(value) : String(value), format: node.format,
    children: [], issues: node.issues.filter(i => matchesProperty(i, node.path, name)) });
  const emitted = new Set<string>();
  order.forEach((name, index) => {
    if (emitted.has(name)) return; emitted.add(name);
    node.props.filter(p => p.name === name).forEach((p, n) => children.push(leaf(name, p.value, index * 1000 + n)));
    for (const child of nested) if (!used.has(child) && propertyName(child.path) === name) { used.add(child); children.push(itemEntry(child, name)); }
  });
  node.props.forEach((p, index) => { if (!emitted.has(p.name)) { emitted.add(p.name); children.push(leaf(p.name, p.value, 100000 + index)); } });
  for (const child of nested) if (!used.has(child)) children.push(itemEntry(child, propertyName(child.path)));
  const type = node.typePath.split(/[>/ ]+/).filter(Boolean).pop() ?? node.typePath;
  // Eigene Befunde des Elements: alle, die keiner Eigenschaft zugeordnet sind (die Eigenschaftsknoten tragen ihre selbst).
  const claimed = new Set(children.filter(c => c.kind === "property").flatMap(c => c.issues));
  return finish({ id: `item:${node.id}`, kind: "item", label: label && label !== type ? `${label}: ${type}` : type, format: node.format, node, children, issues: node.issues.filter(i => !claimed.has(i)) });
}
/** Format → Element → Eigenschaft → Wert; Formate in der Reihenfolge JSON-LD, Microdata, RDFa, Elemente in Quelltextreihenfolge. */
export function buildTree(nodes: readonly SdNode[]): TreeEntry[] {
  const formats = [...new Set([...FORMAT_ORDER.filter(f => nodes.some(n => n.format === f)), ...nodes.map(n => n.format)])];
  return formats.map(format => finish({ id: `format:${format}`, kind: "format", label: FORMAT_LABEL[format] ?? format, format,
    children: nodes.filter(n => n.format === format).map(n => itemEntry(n, "")), issues: [] }));
}
export function flatten(entries: readonly TreeEntry[]): TreeEntry[] { return entries.flatMap(e => [e, ...flatten(e.children)]); }
/** Der Knoten, der den Befund trägt: die Eigenschaft, wenn sie existiert, sonst das Element. */
export function ownerOf(entries: readonly TreeEntry[], issue: SdIssueDto): TreeEntry | undefined {
  const all = flatten(entries);
  return all.find(e => e.kind === "property" && e.issues.includes(issue)) ?? all.find(e => e.issues.includes(issue));
}
/** Anzeige der Prüfart: `Schema.org` oder `Google: <Feature>` (TECH-189). */
export function validationType(issue: SdIssueDto): string {
  return issue.feature ? `Google: ${issue.feature}` : /schema/i.test(issue.validation) ? "Schema.org" : issue.validation;
}
export function summary(entries: readonly TreeEntry[]): { errors: number; warnings: number } {
  const issues = entries.flatMap(e => e.all);
  return { errors: issues.filter(i => severityOf(i) === "error").length, warnings: issues.filter(i => severityOf(i) === "warning").length };
}
/** JSON-LD-Kopie eines Microdata-/RDFa-Elements (UI-045); verschachtelte Elemente stehen unter dem Namen ihrer Eigenschaft. */
export function toJsonLd(node: SdNode, root = true): Record<string, unknown> {
  const type = node.typePath.split(/[>/ ]+/).filter(Boolean).pop() ?? node.typePath;
  const out: Record<string, unknown> = root ? { "@context": "https://schema.org", "@type": type } : { "@type": type };
  for (const prop of node.props) {
    const existing = out[prop.name];
    out[prop.name] = existing === undefined ? prop.value : Array.isArray(existing) ? [...existing, prop.value] : [existing, prop.value];
  }
  for (const child of node.children) {
    const name = propertyName(child.path); const value = toJsonLd(child, false); const existing = out[name];
    out[name] = existing === undefined ? value : Array.isArray(existing) ? [...existing, value] : [existing, value];
  }
  return out;
}
