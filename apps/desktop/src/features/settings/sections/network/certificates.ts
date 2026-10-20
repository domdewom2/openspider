/** Minimal DER reader for displaying an X.509 issuer; trust validation stays in Rust. */
export function certificateIssuer(pem: string): string {
  const bytes = Uint8Array.from(atob(pem.replace(/-----[^-]+-----|\s/g, "")), (c) => c.charCodeAt(0));
  function node(offset: number): { tag: number; start: number; end: number } {
    const tag = bytes[offset++] ?? 0;
    let size = bytes[offset++] ?? 0;
    if (size & 128) {
      const count = size & 127;
      if (!count || count > 4) throw new Error("Invalid DER length");
      size = 0;
      for (let i = 0; i < count; i++) size = size * 256 + (bytes[offset++] ?? 0);
    }
    if (offset + size > bytes.length) throw new Error("Truncated certificate");
    return { tag, start: offset, end: offset + size };
  }
  const root = node(0);
  let current = node(node(root.start).start);
  if (current.tag === 160) current = node(current.end); // optional version
  current = node(current.end); // signature algorithm
  const issuer = node(current.end);
  const names: string[] = [];
  for (let pos = issuer.start; pos < issuer.end;) {
    const set = node(pos);
    const attribute = node(set.start);
    const oid = node(attribute.start);
    const value = node(oid.end);
    const id = [...bytes.slice(oid.start, oid.end)].join(".");
    const label = ({ "85.4.3": "CN", "85.4.10": "O", "85.4.11": "OU", "85.4.6": "C" } as Record<string, string>)[id];
    if (label) names.push(`${label}=${new TextDecoder().decode(bytes.slice(value.start, value.end))}`);
    pos = set.end;
  }
  return names.join(", ");
}
