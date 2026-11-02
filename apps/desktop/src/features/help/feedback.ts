// UI-130: Hilfe → „Feedback geben…“ öffnet das Rückmeldeformular der Beta-Seite im Standardbrowser.
import { app } from "../../ipc/app";
import { client } from "../../ipc/client";

const ORIGIN = "https://openspider.de";

/** Beta-Seite passend zur Oberflächensprache; jede andere Sprache als Deutsch erhält die englische Seite. */
export function feedbackUrl(lang: string, version?: string | null): string {
  const url = new URL(lang === "de" || lang.startsWith("de-") ? "/beta/" : "/en/beta/", ORIGIN);
  const value = version?.trim();
  if (value) url.searchParams.set("version", value.slice(0, 64));
  url.hash = "feedback";
  return url.toString();
}

/** Ohne lesbare Version öffnet sich das Formular trotzdem, nur ohne Vorbelegung. */
export async function openFeedback(lang: string): Promise<void> {
  const version = await app.info().then(info => info.version, () => null);
  const url = feedbackUrl(lang, version);
  if (client.transportName === "tauri") return client.invoke<void>("browser_open", { urls: [url] });
  window.open(url, "_blank", "noopener,noreferrer");
}
