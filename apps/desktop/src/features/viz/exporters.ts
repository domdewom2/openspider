// Dateiausgabe der Visualisierungen: SVG/PNG als Download aus dem Fenster, CSV über `viz.exportCsv` in einen Ordner.
import { useEffect, useState } from "react";
import { client } from "../../ipc/client";
import { usePrefs } from "../../state/prefs";
import { luminance, mix, paletteFromTokens } from "./model/encoding";
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
/**
 * Farben der Zeichenflächen ausschließlich aus den Design-Tokens des aktuellen Themes; daraus abgeleitet (lokale Werte,
 * Bericht v02 „Optik“): Hintergrundverlauf, Plakette, Ausgrau-Ton, Standardschema der Knoten. Ohne gesetzte Tokens
 * (Tests ohne Theme) gelten die festen Standardwerte.
 */
export function readRenderStyle(root: Element = document.documentElement): RenderStyle {
  const css = getComputedStyle(root);
  const v = (name: string) => css.getPropertyValue(name);
  const tok = (name: string, fallback: string) => cssToHex(v(name), fallback);
  const surface = tok("--os-bg-surface", DEFAULT_STYLE.background);
  const app = tok("--os-bg-app", DEFAULT_STYLE.backgroundEdge);
  const border = tok("--os-border", DEFAULT_STYLE.dim);
  const hasTokens = v("--os-bg-surface").trim() !== "";
  const dark = luminance(surface) < 0.2;
  const accent = tok("--os-accent", DEFAULT_STYLE.accent);
  const muted = tok("--os-text-muted", DEFAULT_STYLE.badgeFill);
  return {
    ...DEFAULT_STYLE,
    background: dark ? mix(surface, tok("--os-bg-hover", surface), 0.45) : surface,
    backgroundEdge: dark ? app : mix(app, border, 0.35),
    edge: tok("--os-border-strong", DEFAULT_STYLE.edge),
    text: tok("--os-text", DEFAULT_STYLE.text),
    accent,
    badgeFill: muted,
    badgeText: surface,
    inlink: accent,
    outlink: tok("--os-badge-warning", DEFAULT_STYLE.outlink),
    dim: dark ? mix(border, surface, 0.15) : mix(border, surface, 0.25),
    plaque: surface,
    plaqueBorder: border,
    dark,
    palette: hasTokens
      ? paletteFromTokens(
          {
            surface,
            success: tok("--os-badge-success", "#14652f"),
            danger: tok("--os-badge-danger", "#a82027"),
            warning: tok("--os-badge-warning", "#744900"),
            warningBg: tok("--os-badge-warning-bg", "#fff0cc"),
            accent,
            muted,
            none: tok("--os-status-none", "#7b848e"),
          },
          dark,
        )
      : null,
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
