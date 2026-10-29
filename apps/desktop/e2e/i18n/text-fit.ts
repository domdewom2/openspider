import { expect, type Locator } from "@playwright/test";

/** Inspect every rendered text node, including nested labels and inline fragments. */
export async function textFits(root: Locator, name: string) {
  const clipped = await root.evaluate(root => {
    const hits: { text: string; element: string; x: boolean; y: boolean }[] = [];
    const normalize = (text: string) => text.replace(/\s+/g, " ").trim();
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const text = normalize(node.textContent ?? "");
      const el = node.parentElement;
      if (!text || !el) continue;
      const style = getComputedStyle(el);
      if (style.visibility !== "visible" || el.closest("script, style")) continue;
      const range = document.createRange();
      range.selectNodeContents(node);
      const rects = [...range.getClientRects()].filter(rect => rect.width && rect.height);
      if (!rects.length) continue;
      // Screen-reader-only text has no visual label to truncate.
      let hidden = false;
      for (let parent: HTMLElement | null = el; parent; parent = parent.parentElement) {
        const css = getComputedStyle(parent);
        if (css.clipPath === "inset(50%)" || css.clip === "rect(0px, 0px, 0px, 0px)" || css.opacity === "0") hidden = true;
      }
      if (hidden) continue;
      for (let parent: HTMLElement | null = el; parent && parent !== document.body; parent = parent.parentElement) {
        const css = getComputedStyle(parent);
        const box = parent.getBoundingClientRect();
        // Content outside an intentional scroll viewport is not currently visible.
        if ((["auto", "scroll"].includes(css.overflowY) && rects.every(r => r.bottom <= box.top || r.top >= box.bottom)) ||
            (["auto", "scroll"].includes(css.overflowX) && rects.every(r => r.right <= box.left || r.left >= box.right))) break;
        const fullText = normalize(parent.textContent ?? "");
        const labelled = (parent.getAttribute("aria-labelledby") ?? "").split(/\s+/).map(id => document.getElementById(id)?.textContent ?? "").join(" ");
        const tooltip = (parent.getAttribute("aria-describedby") ?? "").split(/\s+/).map(id => {
          const target = document.getElementById(id);
          return target?.getAttribute("role") === "tooltip" ? target.textContent ?? "" : "";
        }).join(" ");
        const names = [parent.getAttribute("title") ?? "", parent.getAttribute("aria-label") ?? "", labelled, tooltip].map(normalize);
        const namedEllipsis = css.textOverflow === "ellipsis" && names.some(label => label.length > 0 && label === fullText);
        const x = ["hidden", "clip"].includes(css.overflowX) && !namedEllipsis && rects.some(r => r.left < box.left - 1 || r.right > box.right + 1);
        const y = ["hidden", "clip"].includes(css.overflowY) && rects.some(r => r.top < box.top - 1 || r.bottom > box.bottom + 1);
        if (x || y) {
          hits.push({ text, element: `${parent.tagName}.${parent.getAttribute("class") ?? ""}`, x, y });
          break;
        }
      }
    }
    return hits;
  });
  expect(clipped, name).toEqual([]);
}
