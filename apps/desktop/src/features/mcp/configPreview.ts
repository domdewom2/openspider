import type { McpClientKind } from "../../ipc/gen/McpClientKind";
import type { McpClientPreview } from "../../ipc/gen/McpClientPreview";
import { redactText } from "./model";

const HIDDEN = "[redacted]";
const sensitive = /token|secret|password|authorization|api[_-]?key|cookie|credential/i;
const secretFlag = /^--[\w-]*(?:token|secret|password|api[_-]?key|credential)[\w-]*$/i;
type Value = string | number | boolean | null | Value[] | { [key: string]: Value };

function redact(value: Value, key = ""): Value {
  if (sensitive.test(key)) return HIDDEN;
  if (Array.isArray(value)) return value.map((item, i) =>
    i > 0 && typeof value[i - 1] === "string" && secretFlag.test(value[i - 1] as string)
      ? HIDDEN : redact(item));
  if (value !== null && typeof value === "object") return Object.fromEntries(
    Object.entries(value).map(([name, item]) => [name, redact(item, name)]));
  return typeof value === "string" ? redactText(value) : value;
}

/** Conservative TOML reader for client configs. Parse values before redacting arguments, including
 * multiline arrays. Unsupported syntax hides the whole document; raw text is never a fallback.
 * Comments are discarded because they can contain arbitrary credentials too. */
function tomlPreview(source: string): string {
  let pos = 0;
  const fail = (): never => { throw new Error("Unsupported TOML"); };
  const space = () => {
    while (pos < source.length) {
      if (/\s/.test(source[pos]!)) pos++;
      else if (source[pos] === "#") { while (pos < source.length && source[pos] !== "\n") pos++; }
      else break;
    }
  };
  const string = (): string => {
    const quote = source[pos++]!;
    // Multiline string forms need TOML-specific escape/indent handling. Fail closed.
    if (source.slice(pos, pos + 2) === quote.repeat(2)) return fail();
    const start = pos - 1;
    while (pos < source.length) {
      const char = source[pos++];
      if (char === "\n" || char === "\r") return fail();
      if (char === quote) return quote === '"' ? JSON.parse(source.slice(start, pos)) as string : source.slice(start + 1, pos - 1);
      if (char === "\\" && quote === '"') pos++;
    }
    return fail();
  };
  const key = (): string[] => {
    const parts: string[] = [];
    while (true) {
      while (source[pos] === " " || source[pos] === "\t") pos++;
      if (source[pos] === '"' || source[pos] === "'") parts.push(string());
      else {
        const match = /^[\w-]+/.exec(source.slice(pos));
        if (!match) return fail();
        parts.push(match[0]); pos += match[0].length;
      }
      while (source[pos] === " " || source[pos] === "\t") pos++;
      if (source[pos] !== ".") return parts;
      pos++;
    }
  };
  const value = (): Value => {
    space();
    if (source[pos] === '"' || source[pos] === "'") return string();
    if (source[pos] === "[") {
      pos++; space();
      const items: Value[] = [];
      while (source[pos] !== "]") {
        items.push(value()); space();
        if (source[pos] !== ",") break;
        pos++; space();
      }
      if (source[pos++] !== "]") return fail();
      return items;
    }
    if (source[pos] === "{") {
      pos++; space();
      const entries: [string, Value][] = [];
      while (source[pos] !== "}") {
        const name = key().join(".");
        if (source[pos++] !== "=") return fail();
        entries.push([name, value()]); space();
        if (source[pos] !== ",") break;
        pos++; space();
      }
      if (source[pos++] !== "}") return fail();
      return Object.fromEntries(entries);
    }
    const match = /^(?:true|false|[+-]?(?:\d[\d_]*)(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?)\b/.exec(source.slice(pos));
    if (!match) return fail();
    pos += match[0].length;
    return JSON.parse(match[0].replaceAll("_", "").replace(/^\+/, "")) as number | boolean;
  };
  const render = (item: Value): string => {
    if (Array.isArray(item)) return `[${item.map(render).join(", ")}]`;
    if (item !== null && typeof item === "object") return `{ ${Object.entries(item).map(([name, v]) => `${JSON.stringify(name)} = ${render(v)}`).join(", ")} }`;
    return JSON.stringify(item);
  };
  const lines: string[] = [];
  space();
  while (pos < source.length) {
    if (source[pos] === "[") {
      pos++;
      const array = source[pos] === "[";
      if (array) pos++;
      const name = key();
      if (source[pos++] !== "]" || (array && source[pos++] !== "]")) return fail();
      lines.push(`${array ? "[[" : "["}${name.map(p => JSON.stringify(p)).join(".")}${array ? "]]" : "]"}`);
    } else {
      const name = key();
      if (source[pos++] !== "=") return fail();
      lines.push(`${name.map(p => JSON.stringify(p)).join(".")} = ${render(redact(value(), name.join(".")))}`);
    }
    while (source[pos] === " " || source[pos] === "\t" || source[pos] === "\r") pos++;
    if (source[pos] !== undefined && source[pos] !== "\n" && source[pos] !== "#") return fail();
    space();
  }
  return lines.join("\n");
}

export function redactConfig(source: string, kind: McpClientKind): string {
  if (!source.trim()) return "";
  try {
    return kind === "codex" ? tomlPreview(source) : JSON.stringify(redact(JSON.parse(source) as Value), null, 2);
  } catch { return HIDDEN; }
}

/** Build the displayed diff only from sanitized documents, never from the backend's unified diff. */
export function safeClientPreview(preview: McpClientPreview, kind: McpClientKind): { after: string; diff: string } {
  const before = redactConfig(preview.before, kind);
  const after = redactConfig(preview.after, kind);
  const diff = preview.before === preview.after ? "" : ["--- before", "+++ after",
    ...before.split("\n").map(line => `-${line}`), ...after.split("\n").map(line => `+${line}`)].join("\n");
  return { after, diff };
}
