/** Bounded UI text measurement. SEO title/snippet pixel widths remain core-owned
 * (UI-038); this measures only the current display font for clipping/autosizing.
 * DOM measurement also respects the repository's ban on Canvas measureText.
 */
export class DisplayTextMetrics {
  private cache = new Map<string, number>();
  private font = "";
  constructor(private readonly element: HTMLElement) {}
  setFont(font: string) {
    if (font === this.font) return;
    this.font = font;
    this.element.style.font = font;
    this.cache.clear();
  }
  width(text: string): number {
    const cached = this.cache.get(text);
    if (cached !== undefined) return cached;
    this.element.textContent = text;
    // offsetWidth is in unzoomed layout pixels, matching canvas drawing coordinates.
    const width = this.element.offsetWidth;
    if (this.cache.size >= 2048) this.cache.delete(this.cache.keys().next().value!);
    this.cache.set(text, width);
    return width;
  }
}
