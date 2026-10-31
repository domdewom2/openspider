/** Browser-side text geometry. Native form widgets paint their value outside the DOM text tree. */
export function textGeometry(elements: Element[]) {
  const tolerance = 0.5 / devicePixelRatio; // strictly less than one device pixel
  return elements.map(element => {
    const e = element as HTMLElement;
    const r = e.getBoundingClientRect();
    let left = r.left, right = r.right, top = r.top, bottom = r.bottom;
    let scrollableX = false, scrollableY = false;
    for (let p: HTMLElement | null = e; p; p = p.parentElement) {
      const cs = getComputedStyle(p), b = p.getBoundingClientRect();
      // Intersect clipping ancestors at their inner border edge (clientWidth is integer-rounded).
      const scrollX = p !== e && ["auto", "scroll"].includes(cs.overflowX) && p.scrollWidth > p.clientWidth;
      const scrollY = p !== e && ["auto", "scroll"].includes(cs.overflowY) && p.scrollHeight > p.clientHeight;
      // Scrollable ancestors intentionally show a movable excerpt; hidden/clip ancestors cannot reveal text.
      scrollableX ||= scrollX; scrollableY ||= scrollY;
      if (cs.overflowX !== "visible" && !scrollableX) {
        left = Math.max(left, b.left + parseFloat(cs.borderLeftWidth));
        right = Math.min(right, b.right - parseFloat(cs.borderRightWidth));
      }
      if (cs.overflowY !== "visible" && !scrollableY) {
        top = Math.max(top, b.top + parseFloat(cs.borderTopWidth));
        bottom = Math.min(bottom, b.bottom - parseFloat(cs.borderBottomWidth));
      }
    }
    const rects: DOMRect[] = [];
    if (!e.matches("input, select, textarea")) {
      const walker = document.createTreeWalker(e, NodeFilter.SHOW_TEXT);
      for (let node = walker.nextNode(); node; node = walker.nextNode()) {
        if (!node.textContent?.trim() || (node.parentElement && getComputedStyle(node.parentElement).visibility === "hidden")) continue;
        const range = document.createRange();
        range.selectNodeContents(node);
        // Per-node Range rather than the element's rounded scroll dimensions.
        rects.push(range.getBoundingClientRect());
      }
    }
    const vertical = rects.some(b => b.top < top - tolerance || b.bottom > bottom + tolerance);
    const horizontal = rects.some(b => b.left < left - tolerance || b.right > right + tolerance);
    return { vertical, horizontal, visible: right > left && bottom > top, bounds: { left, right, top, bottom },
      textRects: rects.map(b => ({ left: b.left, right: b.right, top: b.top, bottom: b.bottom })), tolerance };
  });
}

/** Verbindet ausschließlich statische Testfunktionen für einen atomaren Browser-Aufruf.
 * Der Helfer wird wie evaluateAll serialisiert und funktioniert auch auf about:blank.
 */
export function withTextGeometry<A, R>(measure: (arg: A, geometry: typeof textGeometry) => R): (arg: A) => R {
  return new Function("arg", `return (${measure.toString()})(arg, ${textGeometry.toString()});`) as (arg: A) => R;
}
