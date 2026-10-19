# M2-U26 – Integrationen-UI

Dialog „API-Zugriff“ mit Verbindungsstatus (INTEG-001), Verbindungsdialoge für GA4 (INTEG-090), Search Console
(INTEG-110), PageSpeed Insights (INTEG-130) sowie Majestic/Ahrefs/Moz mit Ebenen (INTEG-150), API-Fortschritt
(INTEG-007), APIs-Modus als Kopfleisten-Variante (INTEG-012) und Zahnrad je Quelle (UI-070).
Alle Kernaufrufe laufen über die generierten Wrapper `ipc/api.ts` und `ipc/crawl.ts` (Vertrag M2.md 3.6a).

## Einbindung für andere Pakete

- **Hauptfenster:** `IntegrationsHost` (Dialog, Aktionen, Menübeiträge) und `IntegrationsHeaderSlot`
  (API-Fortschritt, im APIs-Modus die Listeneingabe) sind in `App.tsx` gemountet (K2-61). U03 verschiebt den
  `IntegrationsHeaderSlot` beim Merge in seine Kopfleiste.
- **Modus:** Aktion `mode.apis` schaltet die Variante um; `setApiMode(false)` beim Wechsel in einen anderen Modus
  (U03/U04). Die Kopfleiste meldet die Start-URL über `setCrawlUrl(url)` für den Property-Hinweis (INTEG-110).
- **Seitenleiste „API“ (U22, UI-070):** `<ApiSourceGear kind="ahrefs" />` je Zeile öffnet den Dialog genau dieser
  Quelle; alternativ Aktion `integrations.open.<kind>` bzw. `openApiAccess(kind)`.
- **Property-Listen:** Der Command-Vertrag kennt noch keine GA4-Konten/Properties/Streams und keine
  Search-Console-Sites. `registerPropertyCatalog({ ga4, gsc })` nimmt einen Lader an, sobald ein Kern-Command dafür
  existiert; bis dahin bietet der Dialog die manuelle Eingabe der Kennung.

## Tests

- vitest: `logic.test.ts` (reine Logik), `integrations.test.tsx` (Komponenten gegen den Mock-Transport).
- Playwright: `e2e/integrations/dialog.spec.ts` (Vertragsseite mit Mock-Kern), `e2e/integrations/bridge.spec.ts`
  (echte `os-app-bridge`).
