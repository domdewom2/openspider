# OpenSpider Projektwebsite · M3-W01

`site/` enthält die direkt auslieferbare Website: Deutsch unter `/`, Englisch unter
`/en/`, jeweils Funktionen, Download/Installation, FAQ, Impressum und Datenschutz und
404-Seite. Gemeinsames CSS, Bilder und Icons liegen unter `site/assets/`.
Kein Build-Schritt, keine Laufzeitabhängigkeiten, kein ausführbares Browser-JavaScript.
Das bestehende Verzeichnis `placeholder/` bleibt unverändert.

## Auslieferung

Den **Inhalt** von `website/site/` einschließlich Unterordnern nach
`/var/www/openspider.de/public/` kopieren, zum Beispiel vom Repository-Verzeichnis:

```sh
cp -R website/site/. /var/www/openspider.de/public/
```

Caddy muss dieses Verzeichnis als Root mit `file_server` ausliefern.
Canonical-URLs setzen HTTPS und die Domain `openspider.de` voraus.
Die Verzeichnis-URLs verwenden abschließende Schrägstriche. Für unbekannte URLs
muss Caddy die mitgelieferte `/404.html` beziehungsweise `/en/404.html` als
Fehlerdokument mit HTTP-Status 404 ausliefern; `file_server` wählt diese Dateien
nicht automatisch aus. Diese Serverkonfiguration ist beim Betreiber vorzunehmen.
Lokale Vorschau: `python3 -m http.server 8000 --directory website/site`.

## Inhalt und Quellen

Ausschließlich Repo-Inhalte: `README.md`, `docs/ARCHITECTURE.md`, `docs/spec/*`
und `docs/plan/M2.md`. Crawl und Analyse-Tabs: Spec 01–03 und 05;
Exporte, Vergleich und Aufgabenplanung: Spec 06 (OUT-001 ff., OUT-140 ff.,
OUT-200 ff.); Change Detection: Spec 03 (TECH-300 ff.); MCP: Spec 10
(AGENT-001 ff.). Plattformen und Voraussetzungen: Architektur sowie Spec 06,
OUT-260–267 und OUT-273. Linux wird ausdrücklich auf Kern/CLI beschränkt;
Linux-Desktop und Installationspakete werden nicht unterstützt.
Keine Leistungszahlen oder Veröffentlichungstermine. Der dokumentierte
Funktionsumfang wird auf der Funktionsseite vom Umfang einzelner Beta-Builds abgegrenzt.

## Prüfung

```sh
node website/check-site.mjs
bash scripts/check-neutral.sh
```

Der Node-Check benötigt keine Abhängigkeiten. Er prüft alle 14 HTML-Seiten,
interne Links einschließlich Sprungmarken, Assets, erlaubte externe Links,
eindeutige Titel/Beschreibungen, Canonicals, gegenseitige hreflang-Paare,
Sprachwechsel, Überschriften, alt-Attribute, Social-Metadaten, JSON-LD,
Sitemap, robots.txt, OG-Bildmaße und das Gewicht beider Startseiten.
Unkomprimiert einschließlich CSS, Logo und aller referenzierten Icons:
DE **133.888 Byte**, EN **133.643 Byte**; jeweils unter 200.000 Byte.
Das nur in Metadaten referenzierte OG-Bild wird beim Seitenaufruf nicht geladen.
Der Check wurde zusätzlich mit absichtlich beschädigten Links, Bildern,
alt-Attributen, hreflang und h1 geprüft; alle Fehler wurden erkannt.

Zusätzlich lokal in Chrome geprüft: alle 14 Seiten bei 320 und 1280 px,
jeweils hell/dunkel (56 Kombinationen). Kein horizontaler Überlauf, keine
fehlenden Bilder, geprüfte Textkontraste auf WCAG-AA-Niveau, Skip-Link bei
Fokus sichtbar. Die beiden Startseiten und das OG-Bild wurden visuell gesichtet.
Dies ersetzt keine vollständige manuelle Prüfung mit Screenreader.

## Offene Punkte vor Veröffentlichung

- Impressum und Datenschutzerklärung sind aus evnxt.de/wojcik.de übernommen und an openspider.de angepasst
  (keine Cookies, kein Tracking, Zugriffsprotokolle bis neun Wochen, Hoster Hetzner). Brand-Dateien sind eingesetzt;
  das OG-Bild zeigt noch das alte Logo und wird vor der Veröffentlichung neu erzeugt.
- Echte Screenshots ergänzen; die Website zeigt derzeit ausschließlich das Repo-Logo.
- Beta nach Veröffentlichung auf GitHub Releases verlinken und Download-Button aktivieren.
- Caddy-Fehlerseiten wie oben anbinden und HTTPS/404 nach dem Kopieren prüfen.
