// Dateiausgabe der Visualisierungen: SVG/PNG als Download aus dem Fenster, CSV über `viz.exportCsv` in einen Ordner.
import { useEffect, useState } from "react";
import { client } from "../../ipc/client";
import { usePrefs } from "../../state/prefs";
import { cssToHex, DEFAULT_STYLE, type RenderStyle } from "./model/render";

/** Speichert einen Blob über einen Download-Link (Name ohne Pfad). */
export function saveBlob(name: string, blob: Blob, doc: Document = document): void {
  const url = URL.createObjectURL(blob);
  const a = doc.createElement("a");
  a.href = url;
  a.download = name;
  a.rel = "noopener";
  a.style.display = "none";
  doc.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

export function svgBlob(svg: string): Blob {
  return new Blob([svg], { type: "image/svg+xml" });
}

/** Zielordner für `nodes.csv`/`edges.csv`: in der App über den nativen Ordnerdialog. */
export async function chooseFolder(title: string): Promise<string | null> {
  return client.invoke<string | null>("dialog.open", { options: { title, directory: true, multiple: false } });
}

/** Farben der Zeichenfläche aus den Design-Tokens (Theme, Akzent); bei Theme-Wechsel neu gelesen. */
export function readRenderStyle(root: Element = document.documentElement): RenderStyle {
  const css = getComputedStyle(root);
  const v = (name: string) => css.getPropertyValue(name);
  return {
    ...DEFAULT_STYLE,
    background: cssToHex(v("--os-bg-surface"), DEFAULT_STYLE.background),
    edge: cssToHex(v("--os-border-strong"), DEFAULT_STYLE.edge),
    text: cssToHex(v("--os-text"), DEFAULT_STYLE.text),
    accent: cssToHex(v("--os-accent"), DEFAULT_STYLE.accent),
    badgeFill: cssToHex(v("--os-text-muted"), DEFAULT_STYLE.badgeFill),
    badgeText: cssToHex(v("--os-bg-surface"), DEFAULT_STYLE.badgeText),
  };
}

export function useRenderStyle(lineWidth: number): RenderStyle {
  const appearance = usePrefs((s) => s.appearance);
  const [style, setStyle] = useState<RenderStyle>(() => ({ ...readRenderStyle(), lineWidth }));
  useEffect(() => {
    // Tokens werden nach dem Speichern der Einstellungen gesetzt; einen Frame später lesen.
    const id = setTimeout(() => setStyle({ ...readRenderStyle(), lineWidth }), 0);
    return () => clearTimeout(id);
  }, [appearance, lineWidth]);
  return style;
}
