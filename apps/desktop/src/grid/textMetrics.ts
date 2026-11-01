/** Bounded UI text measurement. SEO title/snippet pixel widths remain core-owned
 * (UI-038); this measures only the current display font for clipping/autosizing.
 * DOM measurement also respects the repository's ban on Canvas measureText.
 */
const PRINTABLE_ASCII = /^[\x20-\x7e]*$/;
export class DisplayTextMetrics {
  private cache = new Map<string, number>();
  /** Advance per printable ASCII character for the current font (at most 95 entries). */
  private advances = new Map<string, number>();
  private font = "";
  constructor(private readonly element: HTMLElement) {}
  setFont(font: string) {
    if (font === this.font) return;
    this.font = font;
    this.element.style.font = font;
    this.cache.clear();
    this.advances.clear();
  }
  /** UI-144: WebKit keeps text-width data for every distinct string it lays out, so
   * laying out each new cell value grows renderer memory during long sessions.
   * Text whose upper width bound fits is therefore not laid out as a whole. The
   * bound sums per-character advances (each character laid out alone once per
   * font, so without kerning or ligatures; `offsetWidth` + 1 px covers its
   * rounding) plus 0.5 px per character for kerning and 1 px for the rounded
   * `offsetWidth` of the whole text; it applies to printable ASCII only. When it
   * fits, `width(text) <= width` holds as well, so clipping is unchanged. */
  fits(text: string, width: number): boolean {
    const cached = this.cache.get(text);
    if (cached !== undefined) return cached <= width;
    let bound = 1;
    for (let i = 0; i < text.length; i++) {
      const advance = this.advances.get(text[i]!);
      if (advance === undefined) return false;
      bound += advance + 0.5;
    }
    return bound <= width;
  }
  private learn(texts: readonly string[]): void {
    const missing = new Set<string>();
    for (const text of texts) {
      if (!PRINTABLE_ASCII.test(text)) continue;
      for (const c of text) if (!this.advances.has(c)) missing.add(c);
    }
    if (!missing.size) return;
    const chars = [...missing];
    const fragment = document.createDocumentFragment();
    const nodes = chars.map(c => {
      const span = document.createElement("span");
      span.style.display = "block";
      span.style.width = "max-content";
      span.textContent = c;
      fragment.appendChild(span);
      return span;
    });
    this.element.replaceChildren(fragment);
    const widths = nodes.map(node => node.offsetWidth);
    for (let i = 0; i < chars.length; i++) this.advances.set(chars[i]!, widths[i]! + 1);
  }
  /** Batch misses for a frame: all DOM writes precede width reads. The same
   * binary-search candidates as ellipsis() are measured together per round. */
  prepare(requests: readonly { text: string; width: number }[]): void {
    const batch = (texts: readonly string[]) => {
      const missing = [...new Set(texts)].filter(text => !this.cache.has(text));
      if (!missing.length) return;
      const fragment = document.createDocumentFragment();
      const nodes = missing.map(text => {
        const span = document.createElement("span");
        span.style.display = "block";
        span.style.width = "max-content";
        span.textContent = text;
        fragment.appendChild(span);
        return span;
      });
      this.element.replaceChildren(fragment);
      const widths = nodes.map(node => node.offsetWidth);
      for (let i = 0; i < missing.length; i++) this.remember(missing[i]!, widths[i]!);
    };
    this.learn(requests.map(r => r.text));
    const open = requests.filter(r => !this.fits(r.text, r.width));
    batch(open.map(r => r.text));
    const searches = open.filter(r => this.width(r.text) > r.width).map(r => ({ ...r, lo: 0, hi: r.text.length }));
    while (searches.some(r => r.lo < r.hi)) {
      batch(searches.filter(r => r.lo < r.hi).map(r => r.text.slice(0, Math.ceil((r.lo + r.hi) / 2)) + "…"));
      for (const r of searches) {
        if (r.lo >= r.hi) continue;
        const mid = Math.ceil((r.lo + r.hi) / 2);
        if (this.width(r.text.slice(0, mid) + "…") <= r.width) r.lo = mid;
        else r.hi = mid - 1;
      }
    }
  }
  private remember(text: string, width: number): void {
    if (this.cache.size >= 2048) this.cache.delete(this.cache.keys().next().value!);
    this.cache.set(text, width);
  }
  width(text: string): number {
    const cached = this.cache.get(text);
    if (cached !== undefined) return cached;
    this.element.textContent = text;
    // offsetWidth is in unzoomed layout pixels, matching canvas drawing coordinates.
    const width = this.element.offsetWidth;
    this.remember(text, width);
    return width;
  }
}
