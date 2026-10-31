# Begriffsliste der deutschen Oberfläche (M2-U34, Befund A02-04)

Stand 28.10.2026. Vorschlag für einen Spec-Nachtrag zu UI-121 (Planer). Grundlage: Konvention 05-ui „Spalten- und Filterschlüssel“ – der Kern
führt die englischen Namen als Schlüssel, die Oberfläche zeigt sie über i18n übersetzt; englisch bleiben nur Exporte, kanonische Dateinamen und
CLI (UI-121). Planer-Festlegung K2-75: Ohne Spec-Liste bewusst englischer Fachbegriffe erscheinen Tab-Namen, Filternamen, Spaltenköpfe, Issue-Texte
und Bedienelemente deutsch; englisch bleiben nur die Ausnahmen (a)–(c) am Ende dieser Datei.

**Umsetzung:** Kataloge `apps/desktop/src/i18n/canon/i18n/{de,en}.ftl` (Tabs, Filter, Spalten des Kerns; Schlüssel `canonKey()` in
`apps/desktop/src/i18n/canon.ts`, z. B. `Client Error (4xx)` → `filter-client-error-4xx`), Detail-Tabs `features/detail/frame/i18n`,
Seitenleisten-Tabs `features/sidebar/frame/i18n`. Schwellen in Filternamen stehen als `{n}` und kommen aus der vom Kern aufgelösten Kennung.
Die Prüfung `apps/desktop/e2e/polish/language.spec.ts` liest die Ausnahmeliste aus dieser Datei (Abschnitt „Ausnahmen“, Spalte 1 in Backticks).

## Top-Tabs (UI-004)

| Schlüssel (Spec-Name) | Deutsche Anzeige |
|---|---|
| Internal | Intern |
| External | Extern |
| Security | Sicherheit |
| Response Codes | Antwortcodes |
| URL | URL |
| Page Titles | Seitentitel |
| Meta Description | Meta-Beschreibung |
| Meta Keywords | Meta-Keywords |
| H1 | H1 |
| H2 | H2 |
| Content | Inhalt |
| Images | Bilder |
| Canonicals | Canonicals |
| Pagination | Paginierung |
| Directives | Direktiven |
| Hreflang | Hreflang |
| JavaScript | JavaScript |
| Links | Links |
| Structured Data | Strukturierte Daten |
| Sitemaps | Sitemaps |
| PageSpeed | PageSpeed |
| Mobile | Mobil |
| Accessibility | Barrierefreiheit |
| Custom Search | Eigene Suche |
| Custom Extraction | Eigene Extraktion |
| Custom JavaScript | Eigenes JavaScript |
| Analytics | Analytics |
| Search Console | Search Console |
| Validation | Validierung |
| Link Metrics | Link-Metriken |
| AMP | AMP |
| Change Detection | Änderungserkennung |

## Detail-Tabs (UI-030)

| Schlüssel | Englisch | Deutsche Anzeige |
|---|---|---|
| `urlDetails` | URL Details | URL-Details |
| `inlinks` | Inlinks | Eingehende Links |
| `outlinks` | Outlinks | Ausgehende Links |
| `images` | Image Details | Bilddetails |
| `resources` | Resources | Ressourcen |
| `serp` | SERP Snippet | SERP-Snippet |
| `rendered` | Rendered Page | Gerenderte Seite |
| `consoleLog` | Chrome Console Log | Chrome-Konsolenprotokoll |
| `source` | View Source | Quelltext |
| `headers` | HTTP Headers | HTTP-Header |
| `cookies` | Cookies | Cookies |
| `duplicates` | Duplicate Details | Duplikate |
| `structuredData` | Structured Data Details | Strukturierte Daten |
| `lighthouse` | Lighthouse Details | Lighthouse-Details |
| `spelling` | Spelling & Grammar Details | Rechtschreibung und Grammatik |
| `a11y` | Accessibility Details | Barrierefreiheit |
| `nGrams` | N-Grams | N-Gramme |
| `crawlPath` | Crawl Path | Crawl-Pfad |

## Seitenleisten-Tabs (UI-060)

| Schlüssel | Englisch | Deutsche Anzeige |
|---|---|---|
| `overview` | Overview | Übersicht |
| `issues` | Issues | Befunde |
| `siteStructure` | Site Structure | Seitenstruktur |
| `segments` | Segments | Segmente |
| `responseTimes` | Response Times | Antwortzeiten |
| `api` | API | API |
| `spellingGrammar` | Spelling & Grammar | Rechtschreibung & Grammatik |
| `semanticSearch` | Semantic Search | Semantische Suche |

## Filter der Top-Tabs

Filter der Accessibility-Regeln (axe-Regelkennungen) übersetzt der Katalog `features/detail/a11y/i18n` (unverändert).

| Tab | Filter (Label-Vorlage des Kerns) | Deutsche Anzeige |
|---|---|---|
| Internal | All | Alle |
| Internal | HTML | HTML |
| Internal | JavaScript | JavaScript |
| Internal | CSS | CSS |
| Internal | Images | Bilder |
| Internal | PDF | PDF |
| Internal | Unknown | Unbekannt |
| Internal | Flash | Flash |
| Internal | Other | Sonstige |
| External | All | Alle |
| External | HTML | HTML |
| External | JavaScript | JavaScript |
| External | CSS | CSS |
| External | Images | Bilder |
| External | PDF | PDF |
| External | Flash | Flash |
| External | Other | Sonstige |
| External | Unknown | Unbekannt |
| Security | All | Alle |
| Security | HTTP URLs | HTTP-URLs |
| Security | HTTPS URLs | HTTPS-URLs |
| Security | Mixed Content | Gemischte Inhalte |
| Security | Form URL Insecure | Formular-URL unsicher |
| Security | Form on HTTP URL | Formular auf HTTP-URL |
| Security | Unsafe Cross-Origin Links | Unsichere Cross-Origin-Links |
| Security | Protocol-Relative Resource Links | Protokollrelative Ressourcen-Links |
| Security | Missing HSTS Header | HSTS-Header fehlt |
| Security | Missing Content-Security-Policy Header | Content-Security-Policy-Header fehlt |
| Security | Missing X-Content-Type-Options Header | X-Content-Type-Options-Header fehlt |
| Security | Missing X-Frame-Options Header | X-Frame-Options-Header fehlt |
| Security | Missing Secure Referrer-Policy Header | Sicherer Referrer-Policy-Header fehlt |
| Security | Bad Content Type | Fehlerhafter Inhaltstyp |
| Response Codes | All | Alle |
| Response Codes | Blocked by Robots.txt | Durch robots.txt blockiert |
| Response Codes | No Response | Keine Antwort |
| Response Codes | Success (2XX) | Erfolg (2XX) |
| Response Codes | Redirection (3XX) | Weiterleitung (3XX) |
| Response Codes | Redirection (Meta Refresh) | Weiterleitung (Meta-Refresh) |
| Response Codes | Redirect Chain | Weiterleitungskette |
| Response Codes | Redirect Loop | Weiterleitungsschleife |
| Response Codes | Client Error (4XX) | Client-Fehler (4XX) |
| Response Codes | Server Error (5XX) | Serverfehler (5XX) |
| Response Codes | Other | Sonstige |
| Response Codes | Internal All | Intern: Alle |
| Response Codes | Internal Blocked by Robots.txt | Intern: Durch robots.txt blockiert |
| Response Codes | Internal No Response | Intern: Keine Antwort |
| Response Codes | Internal Success (2XX) | Intern: Erfolg (2XX) |
| Response Codes | Internal Redirection (3XX) | Intern: Weiterleitung (3XX) |
| Response Codes | Internal Redirection (Meta Refresh) | Intern: Weiterleitung (Meta-Refresh) |
| Response Codes | Internal Redirect Chain | Intern: Weiterleitungskette |
| Response Codes | Internal Redirect Loop | Intern: Weiterleitungsschleife |
| Response Codes | Internal Client Error (4XX) | Intern: Client-Fehler (4XX) |
| Response Codes | Internal Server Error (5XX) | Intern: Serverfehler (5XX) |
| Response Codes | Internal Other | Intern: Sonstige |
| Response Codes | External All | Extern: Alle |
| Response Codes | External Blocked by Robots.txt | Extern: Durch robots.txt blockiert |
| Response Codes | External No Response | Extern: Keine Antwort |
| Response Codes | External Success (2XX) | Extern: Erfolg (2XX) |
| Response Codes | External Redirection (3XX) | Extern: Weiterleitung (3XX) |
| Response Codes | External Redirection (Meta Refresh) | Extern: Weiterleitung (Meta-Refresh) |
| Response Codes | External Redirect Chain | Extern: Weiterleitungskette |
| Response Codes | External Redirect Loop | Extern: Weiterleitungsschleife |
| Response Codes | External Client Error (4XX) | Extern: Client-Fehler (4XX) |
| Response Codes | External Server Error (5XX) | Extern: Serverfehler (5XX) |
| Response Codes | External Other | Extern: Sonstige |
| Response Codes | Blocked Resource | Blockierte Ressource |
| Response Codes | Internal Blocked Resource | Intern: Blockierte Ressource |
| Response Codes | External Blocked Resource | Extern: Blockierte Ressource |
| Response Codes | Redirection (JavaScript) | Weiterleitung (JavaScript) |
| Response Codes | Internal Redirection (JavaScript) | Intern: Weiterleitung (JavaScript) |
| Response Codes | External Redirection (JavaScript) | Extern: Weiterleitung (JavaScript) |
| Response Codes | Redirection (HTTP Refresh) | Weiterleitung (HTTP-Refresh) |
| Response Codes | Internal Redirection (HTTP Refresh) | Intern: Weiterleitung (HTTP-Refresh) |
| Response Codes | External Redirection (HTTP Refresh) | Extern: Weiterleitung (HTTP-Refresh) |
| URL | All | Alle |
| URL | Non ASCII Characters | Nicht-ASCII-Zeichen |
| URL | Underscores | Unterstriche |
| URL | Uppercase | Großbuchstaben |
| URL | Multiple Slashes | Mehrfache Schrägstriche |
| URL | Contains A Space | Enthält Leerzeichen |
| URL | Parameters | Parameter |
| URL | Over {url.max_chars} Characters | Über {n} Zeichen |
| URL | Repetitive Path | Wiederholter Pfad |
| URL | Internal Search | Interne Suche |
| URL | Broken Bookmark | Defekte Sprungmarke |
| URL | GA Tracking Parameters | GA-Tracking-Parameter |
| Page Titles | All | Alle |
| Page Titles | Missing | Fehlt |
| Page Titles | Duplicate | Duplikat |
| Page Titles | Over {title.max_chars} Characters | Über {n} Zeichen |
| Page Titles | Below {title.min_chars} Characters | Unter {n} Zeichen |
| Page Titles | Multiple | Mehrfach |
| Page Titles | Over {title.max_px} Pixels | Über {n} Pixel |
| Page Titles | Below {title.min_px} Pixels | Unter {n} Pixel |
| Page Titles | Same as H1 | Identisch mit H1 |
| Page Titles | Outside <head> | Außerhalb von <head> |
| Meta Description | All | Alle |
| Meta Description | Missing | Fehlt |
| Meta Description | Duplicate | Duplikat |
| Meta Description | Over {meta_description.max_chars} Characters | Über {n} Zeichen |
| Meta Description | Below {meta_description.min_chars} Characters | Unter {n} Zeichen |
| Meta Description | Multiple | Mehrfach |
| Meta Description | Over {meta_description.max_px} Pixels | Über {n} Pixel |
| Meta Description | Below {meta_description.min_px} Pixels | Unter {n} Pixel |
| Meta Description | Outside <head> | Außerhalb von <head> |
| Meta Keywords | All | Alle |
| Meta Keywords | Missing | Fehlt |
| Meta Keywords | Duplicate | Duplikat |
| Meta Keywords | Multiple | Mehrfach |
| H1 | All | Alle |
| H1 | Missing | Fehlt |
| H1 | Duplicate | Duplikat |
| H1 | Over {h1.max_chars} Characters | Über {n} Zeichen |
| H1 | Multiple | Mehrfach |
| H1 | Alt Text in h1 | Alt-Text in h1 |
| H1 | Non-sequential | Nicht sequenziell |
| H2 | All | Alle |
| H2 | Missing | Fehlt |
| H2 | Duplicate | Duplikat |
| H2 | Over {h2.max_chars} Characters | Über {n} Zeichen |
| H2 | Multiple | Mehrfach |
| H2 | Non-sequential | Nicht sequenziell |
| Content | All | Alle |
| Content | Exact Duplicates | Exakte Duplikate |
| Content | Low Content Pages | Seiten mit wenig Inhalt |
| Content | Near Duplicates | Beinahe-Duplikate |
| Content | Soft 404 Pages | Soft-404-Seiten |
| Content | Lorem Ipsum Placeholder | Lorem-ipsum-Platzhalter |
| Content | Readability Difficult | Lesbarkeit schwer |
| Content | Readability Very Difficult | Lesbarkeit sehr schwer |
| Content | Spelling Errors | Rechtschreibfehler |
| Content | Grammar Errors | Grammatikfehler |
| Images | All | Alle |
| Images | Over {image.max_kb} KB | Über {n} KB |
| Images | Missing Alt Text | Alt-Text fehlt |
| Images | Missing Alt Attribute | Alt-Attribut fehlt |
| Images | Alt Text Over {image.alt_max_chars} Characters | Alt-Text über {n} Zeichen |
| Images | Background Images | Hintergrundbilder |
| Images | Missing Size Attributes | Größenattribute fehlen |
| Images | Incorrectly Sized Images | Falsch dimensionierte Bilder |
| Canonicals | All | Alle |
| Canonicals | Contains Canonical | Enthält Canonical |
| Canonicals | Self Referencing | Selbstreferenzierend |
| Canonicals | Canonicalised | Kanonisiert |
| Canonicals | Missing | Fehlt |
| Canonicals | Multiple | Mehrfach |
| Canonicals | Multiple Conflicting | Mehrfach, widersprüchlich |
| Canonicals | Non-Indexable Canonical | Nicht indexierbares Canonical |
| Canonicals | Canonical Is Relative | Canonical ist relativ |
| Canonicals | Unlinked | Nicht verlinkt |
| Canonicals | Invalid Attribute In Annotation | Ungültiges Attribut in Annotation |
| Canonicals | Contains Fragment URL | Enthält Fragment-URL |
| Canonicals | Outside <head> | Außerhalb von <head> |
| Pagination | All | Alle |
| Pagination | Contains Pagination | Enthält Paginierung |
| Pagination | First Page | Erste Seite |
| Pagination | Paginated 2+ Pages | Paginierte Seiten 2+ |
| Pagination | Pagination URL Not In Anchor Tag | Paginierungs-URL nicht in Anker-Tag |
| Pagination | Non-200 Pagination URLs | Paginierungs-URLs ohne 200 |
| Pagination | Unlinked Pagination URLs | Nicht verlinkte Paginierungs-URLs |
| Pagination | Non-Indexable | Nicht indexierbar |
| Pagination | Multiple Pagination URLs | Mehrere Paginierungs-URLs |
| Pagination | Pagination Loop | Paginierungsschleife |
| Pagination | Sequence Error | Sequenzfehler |
| Directives | All | Alle |
| Directives | Index | Index |
| Directives | Noindex | Noindex |
| Directives | Follow | Follow |
| Directives | Nofollow | Nofollow |
| Directives | None | None |
| Directives | Refresh | Refresh |
| Directives | NoArchive | NoArchive |
| Directives | NoSnippet | NoSnippet |
| Directives | Max-Snippet | Max-Snippet |
| Directives | Max-Image-Preview | Max-Image-Preview |
| Directives | Max-Video-Preview | Max-Video-Preview |
| Directives | NoImageIndex | NoImageIndex |
| Directives | NoTranslate | NoTranslate |
| Directives | Unavailable_After | Unavailable_After |
| Directives | Outside <head> | Außerhalb von <head> |
| Hreflang | All | Alle |
| Hreflang | Contains Hreflang | Enthält Hreflang |
| Hreflang | Non-200 Hreflang URLs | Hreflang-URLs ohne 200 |
| Hreflang | Missing Return Links | Rückverweise fehlen |
| Hreflang | Incorrect Language & Region Codes | Falsche Sprach- & Regionscodes |
| Hreflang | Missing | Fehlt |
| Hreflang | Unlinked Hreflang URLs | Nicht verlinkte Hreflang-URLs |
| Hreflang | Inconsistent Language & Region Return Links | Inkonsistente Sprach- & Regions-Rückverweise |
| Hreflang | Non-Canonical Return Links | Nicht-kanonische Rückverweise |
| Hreflang | Noindex Return Links | Noindex-Rückverweise |
| Hreflang | Multiple Entries | Mehrfache Einträge |
| Hreflang | Missing Self Reference | Selbstreferenz fehlt |
| Hreflang | Not Using Canonical | Verwendet nicht das Canonical |
| Hreflang | Missing X-Default | X-Default fehlt |
| Hreflang | Outside <head> | Außerhalb von <head> |
| JavaScript | All | Alle |
| JavaScript | Pages With Blocked Resources | Seiten mit blockierten Ressourcen |
| JavaScript | Contains JavaScript Links | Enthält JavaScript-Links |
| JavaScript | Contains JavaScript Content | Enthält JavaScript-Inhalte |
| JavaScript | Noindex Only In Original HTML | Noindex nur im Original-HTML |
| JavaScript | Nofollow Only In Original HTML | Nofollow nur im Original-HTML |
| JavaScript | Canonical Only In Rendered HTML | Canonical nur im gerenderten HTML |
| JavaScript | Canonical Mismatch | Canonical-Abweichung |
| JavaScript | Page Title Only In Rendered HTML | Seitentitel nur im gerenderten HTML |
| JavaScript | Page Title Updated By JavaScript | Seitentitel durch JavaScript geändert |
| JavaScript | Meta Description Only In Rendered HTML | Meta-Beschreibung nur im gerenderten HTML |
| JavaScript | Meta Description Updated By JavaScript | Meta-Beschreibung durch JavaScript geändert |
| JavaScript | H1 Only In Rendered HTML | H1 nur im gerenderten HTML |
| JavaScript | H1 Updated By JavaScript | H1 durch JavaScript geändert |
| JavaScript | Pages With JavaScript Errors | Seiten mit JavaScript-Fehlern |
| Links | All | Alle |
| Links | Pages With High Crawl Depth | Seiten mit hoher Crawl-Tiefe |
| Links | Pages Without Internal Outlinks | Seiten ohne interne ausgehende Links |
| Links | Internal Nofollow Outlinks | Interne ausgehende Nofollow-Links |
| Links | Internal Outlinks With No Anchor Text | Interne ausgehende Links ohne Ankertext |
| Links | Non-Descriptive Anchor Text In Internal Outlinks | Nichtssagender Ankertext in internen ausgehenden Links |
| Links | Pages With High External Outlinks | Seiten mit vielen externen ausgehenden Links |
| Links | Pages With High Internal Outlinks | Seiten mit vielen internen ausgehenden Links |
| Links | Follow & Nofollow Internal Inlinks To Page | Interne eingehende Follow- & Nofollow-Links zur Seite |
| Links | Internal Nofollow Inlinks Only | Nur interne eingehende Nofollow-Links |
| Links | Outlinks To Localhost | Ausgehende Links zu localhost |
| Links | Non-Indexable Page Inlinks Only | Nur eingehende Links von nicht indexierbaren Seiten |
| Links | Pages With Uncrawlable Internal Outlinks | Seiten mit nicht crawlbaren internen ausgehenden Links |
| Structured Data | All | Alle |
| Structured Data | Contains Structured Data | Enthält strukturierte Daten |
| Structured Data | Missing Structured Data | Strukturierte Daten fehlen |
| Structured Data | JSON-LD URLs | JSON-LD-URLs |
| Structured Data | Microdata URLs | Microdata-URLs |
| Structured Data | RDFa URLs | RDFa-URLs |
| Structured Data | Parse Errors | Parsing-Fehler |
| Structured Data | Validation Errors | Validierungsfehler |
| Structured Data | Validation Warnings | Validierungswarnungen |
| Structured Data | Rich Result Validation Errors | Rich-Result-Validierungsfehler |
| Structured Data | Rich Result Validation Warnings | Rich-Result-Validierungswarnungen |
| Sitemaps | All | Alle |
| Sitemaps | URLs In Sitemap | URLs in Sitemap |
| Sitemaps | URLs Not In Sitemap | URLs nicht in Sitemap |
| Sitemaps | Orphan URLs | Verwaiste URLs |
| Sitemaps | Non-Indexable URLs In Sitemap | Nicht indexierbare URLs in Sitemap |
| Sitemaps | URLs In Multiple Sitemaps | URLs in mehreren Sitemaps |
| Sitemaps | XML Sitemap With Over 50k URLs | XML-Sitemap mit über 50k URLs |
| Sitemaps | XML Sitemap With Over 50MB | XML-Sitemap über 50MB |
| PageSpeed | All | Alle |
| PageSpeed | Avoid Enormous Network Payloads | Sehr große Netzwerk-Payloads vermeiden |
| PageSpeed | Document Request Latency | Latenz der Dokumentanfrage |
| PageSpeed | Duplicated JavaScript | Doppeltes JavaScript |
| PageSpeed | Font Display | Schriftanzeige |
| PageSpeed | Forced Reflow | Erzwungener Reflow |
| PageSpeed | Improve Image Delivery | Bildauslieferung verbessern |
| PageSpeed | LCP Request Discovery | LCP-Anfrageerkennung |
| PageSpeed | Layout Shift Culprits | Verursacher von Layout-Verschiebungen |
| PageSpeed | Legacy JavaScript | Veraltetes JavaScript |
| PageSpeed | Minify CSS | CSS minimieren |
| PageSpeed | Minify JavaScript | JavaScript minimieren |
| PageSpeed | Minimize Main-Thread Work | Hauptthread-Arbeit minimieren |
| PageSpeed | Network Dependency Tree | Netzwerk-Abhängigkeitsbaum |
| PageSpeed | Optimize DOM Size | DOM-Größe optimieren |
| PageSpeed | Reduce JavaScript Execution Time | JavaScript-Ausführungszeit reduzieren |
| PageSpeed | Reduce Unused CSS | Ungenutztes CSS reduzieren |
| PageSpeed | Reduce Unused JavaScript | Ungenutztes JavaScript reduzieren |
| PageSpeed | Render Blocking Requests | Renderblockierende Anfragen |
| PageSpeed | Use Efficient Cache Lifetimes | Effiziente Cache-Dauer nutzen |
| Mobile | All | Alle |
| Mobile | Viewport Not Set | Viewport nicht gesetzt |
| Mobile | Content Not Sized Correctly | Inhalt nicht passend dimensioniert |
| Mobile | Illegible Font Size | Unleserliche Schriftgröße |
| Mobile | Unsupported Plugins | Nicht unterstützte Plugins |
| Mobile | Mobile Alternate Link | Mobiler Alternate-Link |
| Accessibility | All | Alle |
| Custom Search | All | Alle |
| Custom Extraction | All | Alle |
| Custom JavaScript | All | Alle |
| Analytics | All | Alle |
| Analytics | Sessions Above 0 | Sitzungen über 0 |
| Analytics | Bounce Rate Above 70% | Absprungrate über 70% |
| Analytics | No GA Data | Keine GA-Daten |
| Analytics | Non-Indexable with GA Data | Nicht indexierbar mit GA-Daten |
| Analytics | Orphan URLs | Verwaiste URLs |
| Search Console | All | Alle |
| Search Console | Clicks Above 0 | Klicks über 0 |
| Search Console | No Search Analytics Data | Keine Search-Analytics-Daten |
| Search Console | Non-Indexable with Search Analytics Data | Nicht indexierbar mit Search-Analytics-Daten |
| Search Console | Orphan URLs | Verwaiste URLs |
| Search Console | URL Is Not on Google | URL ist nicht bei Google |
| Search Console | Indexable URL Not Indexed | Indexierbare URL nicht indexiert |
| Search Console | URL is on Google, But Has Issues | URL ist bei Google, hat aber Probleme |
| Search Console | User-Declared Canonical Not Selected | Vom Nutzer angegebenes Canonical nicht gewählt |
| Search Console | Page Is Not Mobile Friendly | Seite ist nicht mobilfreundlich |
| Search Console | AMP URL Is Invalid | AMP-URL ist ungültig |
| Search Console | Rich Result Invalid | Rich Result ungültig |
| Validation | All | Alle |
| Validation | Invalid HTML Elements In <head> | Ungültige HTML-Elemente in <head> |
| Validation | <head> Not First In <html> Element | <head> nicht zuerst im <html>-Element |
| Validation | Missing <head> Tag | <head>-Tag fehlt |
| Validation | Multiple <head> Tags | Mehrere <head>-Tags |
| Validation | Missing <body> Tag | <body>-Tag fehlt |
| Validation | Multiple <body> Tags | Mehrere <body>-Tags |
| Validation | <body> Element Preceding <html> | <body>-Element vor <html> |
| Validation | HTML Document Over {validation.html_doc_max_bytes}MB | HTML-Dokument über {n}MB |
| Validation | Resource Over {validation.resource_max_bytes}MB | Ressource über {n}MB |
| Link Metrics | All | Alle |

## Spaltenköpfe

| Spalte (Spec-Name) | Deutsche Anzeige |
|---|---|
| % of Total | % vom Gesamtwert |
| 3rd Parties | Drittanbieter |
| AMP Issues | AMP-Probleme |
| AMP Results | AMP-Ergebnisse |
| Accessibility Score | Barrierefreiheits-Score |
| Address | Adresse |
| All Violations | Alle Verstöße |
| Applies Text Compression | Nutzt Textkomprimierung |
| Average Words Per Sentence | Ø Wörter pro Satz |
| Best Practice Violations | Best-Practice-Verstöße |
| CSS Count | CSS-Anzahl |
| CSS Size | CSS-Größe |
| CTR | CTR |
| Canonical Link Element | Canonical-Link-Element |
| Canonical Link Element 1 | Canonical-Link-Element 1 |
| Canonical URL | Canonical-URL |
| Clicks | Klicks |
| Closest Semantically Similar Address | Nächste semantisch ähnliche Adresse |
| Closest Similarity Match | Größte Ähnlichkeit |
| Content | Inhalt |
| Content Type | Inhaltstyp |
| Content Width | Inhaltsbreite |
| Core Web Vitals Assessment | Core-Web-Vitals-Bewertung |
| Coverage | Abdeckung |
| CrUX Cumulative Layout Shift | CrUX Cumulative Layout Shift |
| CrUX Cumulative Layout Shift Category | CrUX Cumulative Layout Shift (Kategorie) |
| CrUX First Contentful Paint (sec) | CrUX First Contentful Paint (s) |
| CrUX First Contentful Paint Category | CrUX First Contentful Paint (Kategorie) |
| CrUX Interaction to Next Paint (ms) | CrUX Interaction to Next Paint (ms) |
| CrUX Interaction to Next Paint Category | CrUX Interaction to Next Paint (Kategorie) |
| CrUX Largest Contentful Paint (sec) | CrUX Largest Contentful Paint (s) |
| CrUX Largest Contentful Paint Category | CrUX Largest Contentful Paint (Kategorie) |
| CrUX Origin Core Web Vitals Assessment | CrUX Origin Core-Web-Vitals-Bewertung |
| CrUX Origin Cumulative Layout Shift | CrUX Origin Cumulative Layout Shift |
| CrUX Origin Cumulative Layout Shift Category | CrUX Origin Cumulative Layout Shift (Kategorie) |
| CrUX Origin First Contentful Paint (sec) | CrUX Origin First Contentful Paint (s) |
| CrUX Origin First Contentful Paint Category | CrUX Origin First Contentful Paint (Kategorie) |
| CrUX Origin Interaction to Next Paint (ms) | CrUX Origin Interaction to Next Paint (ms) |
| CrUX Origin Interaction to Next Paint Category | CrUX Origin Interaction to Next Paint (Kategorie) |
| CrUX Origin Largest Contentful Paint (sec) | CrUX Origin Largest Contentful Paint (s) |
| CrUX Origin Largest Contentful Paint Category | CrUX Origin Largest Contentful Paint (Kategorie) |
| CrUX Origin Time to First Byte (ms) | CrUX Origin Time to First Byte (ms) |
| CrUX Origin Time to First Byte Category | CrUX Origin Time to First Byte (Kategorie) |
| CrUX Time to First Byte (ms) | CrUX Time to First Byte (ms) |
| CrUX Time to First Byte Category | CrUX Time to First Byte (Kategorie) |
| Crawl Allowed | Crawling erlaubt |
| Crawl Depth | Crawl-Tiefe |
| Crawl Timestamp | Crawl-Zeitpunkt |
| Crawled As | Gecrawlt als |
| Cumulative Layout Shift | Cumulative Layout Shift |
| Cumulative Layout Shift Score | Cumulative Layout Shift (Score) |
| DOM Size | DOM-Größe |
| Document Request Latency | Latenz der Dokumentanfrage |
| Duplicated JavaScript Savings (Bytes) | Doppeltes JavaScript – Einsparung (Bytes) |
| Effective Follow | Follow (effektiv) |
| Effective Index | Index (effektiv) |
| Efficient Cache Policy Savings (Bytes) | Effiziente Cache-Richtlinie – Einsparung (Bytes) |
| Errors | Fehler |
| Expected Content Type | Erwarteter Inhaltstyp |
| External Outlinks | Externe ausgehende Links |
| First Contentful Paint Score | First Contentful Paint (Score) |
| First Contentful Paint Time (sec) | First Contentful Paint (Zeit, s) |
| Flesch Reading Ease Score | Flesch-Reading-Ease-Score |
| Folder Depth | Ordnertiefe |
| Font Count | Schriftanzahl |
| Font Display | Schriftanzeige |
| Font Display Savings (ms) | Schriftanzeige – Einsparung (ms) |
| Font Size | Schriftgröße |
| Forced Reflow Savings (ms) | Erzwungener Reflow – Einsparung (ms) |
| Found Via | Gefunden über |
| Google-Selected Canonical | Von Google gewähltes Canonical |
| Grammar Errors | Grammatikfehler |
| H1-1 | H1-1 |
| H1-1 Length | H1-1 Länge |
| H1-2 | H1-2 |
| H1-2 Length | H1-2 Länge |
| H2-1 | H2-1 |
| H2-1 Length | H2-1 Länge |
| H2-2 | H2-2 |
| H2-2 Length | H2-2 Länge |
| HTML Canonical | Canonical (HTML) |
| HTML Count | HTML-Anzahl |
| HTML H1 | H1 (HTML) |
| HTML Meta Description | Meta-Beschreibung (HTML) |
| HTML Meta Robots | Meta Robots (HTML) |
| HTML Size | HTML-Größe |
| HTML Title | Titel (HTML) |
| HTML Word Count | Wortanzahl (HTML) |
| HTML hreflang | hreflang (HTML) |
| HTTP Canonical | Canonical (HTTP) |
| HTTP Version | HTTP-Version |
| HTTP hreflang | hreflang (HTTP) |
| HTTP rel="next" 1 | rel="next" 1 (HTTP) |
| HTTP rel="prev" 1 | rel="prev" 1 (HTTP) |
| Hash | Hashwert |
| IMG Inlinks | Eingehende IMG-Links |
| Image Count | Bildanzahl |
| Image Size | Bildgröße |
| Impressions | Impressionen |
| Improve Image Delivery Savings (Bytes) | Bildauslieferung verbessern – Einsparung (Bytes) |
| Indexability | Indexierbarkeit |
| Indexability Status | Indexierbarkeitsstatus |
| Indexing Allowed | Indexierung erlaubt |
| Inlinks | Eingehende Links |
| Inspection Property | Geprüfte Property |
| JS Word Count % | JS-Wortanzahl % |
| JavaScript Count | JavaScript-Anzahl |
| JavaScript Execution Time (sec) | JavaScript-Ausführungszeit (s) |
| JavaScript Execution Time Category | JavaScript-Ausführungszeit (Kategorie) |
| JavaScript Size | JavaScript-Größe |
| LCP Breakdown Load Delay (ms) | LCP-Aufschlüsselung Ladeverzögerung (ms) |
| LCP Breakdown Load Duration (ms) | LCP-Aufschlüsselung Ladedauer (ms) |
| LCP Breakdown Render Delay (ms) | LCP-Aufschlüsselung Renderverzögerung (ms) |
| LCP Breakdown TTFB (ms) | LCP-Aufschlüsselung TTFB (ms) |
| LCP Request Discovery | LCP-Anfrageerkennung |
| Language | Sprache |
| Largest Contentful Paint Score | Largest Contentful Paint (Score) |
| Largest Contentful Paint Time (sec) | Largest Contentful Paint (Zeit, s) |
| Last Crawl | Letzter Crawl |
| Last-Modified | Last-Modified |
| Layout Shift Culprits | Verursacher von Layout-Verschiebungen |
| Legacy JavaScript Savings (Bytes) | Veraltetes JavaScript – Einsparung (Bytes) |
| Length | Länge |
| Lighthouse Version | Lighthouse-Version |
| Link Score | Link-Score |
| Links Truncated | Links gekürzt |
| Matched Phrase | Gefundene Phrase |
| Matched Robots.txt Line | Passende robots.txt-Zeile |
| Max Potential First Input Delay (ms) | Max. potenzieller First Input Delay (ms) |
| Max Potential First Input Delay Score | Max. potenzieller First Input Delay (Score) |
| Maximum Critical Path Latency (ms) | Max. Latenz des kritischen Pfads (ms) |
| Media Count | Medienanzahl |
| Media Size | Mediengröße |
| Meta Description 1 | Meta-Beschreibung 1 |
| Meta Description 1 Length | Meta-Beschreibung 1 Länge |
| Meta Description 1 Pixel Width | Meta-Beschreibung 1 Pixelbreite |
| Meta Description 2 | Meta-Beschreibung 2 |
| Meta Description 2 Length | Meta-Beschreibung 2 Länge |
| Meta Description 2 Pixel Width | Meta-Beschreibung 2 Pixelbreite |
| Meta Keywords 1 | Meta-Keywords 1 |
| Meta Keywords 1 Length | Meta-Keywords 1 Länge |
| Meta Keywords 2 | Meta-Keywords 2 |
| Meta Keywords 2 Length | Meta-Keywords 2 Länge |
| Meta Refresh | Meta-Refresh |
| Meta Refresh 1 | Meta-Refresh 1 |
| Meta Robots | Meta Robots |
| Minify CSS Savings (Bytes) | CSS minimieren – Einsparung (Bytes) |
| Minify CSS Savings (ms) | CSS minimieren – Einsparung (ms) |
| Minify JavaScript Savings (Bytes) | JavaScript minimieren – Einsparung (Bytes) |
| Minify JavaScript Savings (ms) | JavaScript minimieren – Einsparung (ms) |
| Minimize Main-Thread Work (sec) | Hauptthread-Arbeit minimieren (s) |
| Minimize Main-Thread Work Category | Hauptthread-Arbeit minimieren (Kategorie) |
| Mobile Alternate Link | Mobiler Alternate-Link |
| Mobile Usability | Mobile Nutzerfreundlichkeit |
| Mobile Usability Issues | Probleme mobile Nutzerfreundlichkeit |
| Network Payload Size (Bytes) | Netzwerk-Payload-Größe (Bytes) |
| No. Near Duplicates | Anz. Beinahe-Duplikate |
| No. Semantically Similar | Anz. semantisch ähnlicher |
| Number of Redirects | Anzahl Weiterleitungen |
| Occurrences | Vorkommen |
| Other Count | Anzahl Sonstige |
| Other Size | Größe Sonstige |
| Outlinks | Ausgehende Links |
| PSI Error | PSI-Fehler |
| PSI Status | PSI-Status |
| Page Fetch | Seitenabruf |
| Performance Score | Performance-Score |
| Plugins | Plugins |
| Position | Position |
| Preconnect Candidates Savings (ms) | Preconnect-Kandidaten – Einsparung (ms) |
| Readability | Lesbarkeit |
| Redirect Type | Weiterleitungstyp |
| Redirect URL | Weiterleitungs-URL |
| Reduce Unused CSS Savings (Bytes) | Ungenutztes CSS reduzieren – Einsparung (Bytes) |
| Reduce Unused CSS Savings (ms) | Ungenutztes CSS reduzieren – Einsparung (ms) |
| Reduce Unused JavaScript Savings (Bytes) | Ungenutztes JavaScript reduzieren – Einsparung (Bytes) |
| Reduce Unused JavaScript Savings (ms) | Ungenutztes JavaScript reduzieren – Einsparung (ms) |
| Render Blocking Requests Savings (ms) | Renderblockierende Anfragen – Einsparung (ms) |
| Rendered HTML Canonical | Canonical (gerendertes HTML) |
| Rendered HTML H1 | H1 (gerendertes HTML) |
| Rendered HTML Meta Description | Meta-Beschreibung (gerendertes HTML) |
| Rendered HTML Meta Robots | Meta Robots (gerendertes HTML) |
| Rendered HTML Title | Titel (gerendertes HTML) |
| Rendered HTML Word Count | Wortanzahl (gerendertes HTML) |
| Response Time | Antwortzeit |
| Rich Result Errors | Rich-Result-Fehler |
| Rich Result Features | Rich-Result-Funktionen |
| Rich Result Warnings | Rich-Result-Warnungen |
| Rich Results | Rich Results |
| Rich Results Types | Rich-Results-Typen |
| Rich Results Types Errors | Fehler Rich-Results-Typen |
| Rich Results Warnings | Rich-Results-Warnungen |
| Semantic Relevance Score | Semantischer Relevanz-Score |
| Semantic Similarity Score | Semantischer Ähnlichkeits-Score |
| Sentence Count | Satzanzahl |
| Server Responds Quickly | Server antwortet schnell |
| Sitemap hreflang | hreflang (Sitemap) |
| Sitemaps | Sitemaps |
| Size | Größe |
| Size (bytes) | Größe (Bytes) |
| Speed Index Score | Speed Index (Score) |
| Speed Index Time (sec) | Speed Index (Zeit, s) |
| Spelling Errors | Rechtschreibfehler |
| Status | Status |
| Status Code | Statuscode |
| Summary | Zusammenfassung |
| Tap Targets | Tippziele |
| Text Ratio | Textanteil |
| Third Party Count | Drittanbieter-Anzahl |
| Third Party Size | Drittanbieter-Größe |
| Time to Interactive (sec) | Time to Interactive (s) |
| Time to Interactive Score | Time to Interactive (Score) |
| Title 1 | Titel 1 |
| Title 1 Length | Titel 1 Länge |
| Title 1 Pixel Width | Titel 1 Pixelbreite |
| Title 2 | Titel 2 |
| Title 2 Length | Titel 2 Länge |
| Title 2 Pixel Width | Titel 2 Pixelbreite |
| Total Blocking Time (ms) | Total Blocking Time (ms) |
| Total Blocking Time Score | Total Blocking Time (Score) |
| Total Language Errors | Sprachfehler gesamt |
| Total Page Size (Bytes) | Seitengröße gesamt (Bytes) |
| Total Requests | Anfragen gesamt |
| Total Size Savings (Bytes) | Einsparung Größe gesamt (Bytes) |
| Total Time Savings (ms) | Einsparung Zeit gesamt (ms) |
| Total Transferred (bytes) | Übertragen gesamt (Bytes) |
| Total Types | Typen gesamt |
| Transferred (bytes) | Übertragen (Bytes) |
| Truncated | Gekürzt |
| Type | Typ |
| URL Encoded Address | URL-kodierte Adresse |
| Unique External JS Outlinks | Eindeutige externe ausgehende JS-Links |
| Unique External Outlinks | Eindeutige externe ausgehende Links |
| Unique Inlinks | Eindeutige eingehende Links |
| Unique JS Inlinks | Eindeutige eingehende JS-Links |
| Unique JS Outlinks | Eindeutige ausgehende JS-Links |
| Unique Outlinks | Eindeutige ausgehende Links |
| Unique Types | Eindeutige Typen |
| Use Efficient Cache Lifetimes Savings (Bytes) | Effiziente Cache-Dauer nutzen – Einsparung (Bytes) |
| User Timing Marks and Measures | User-Timing-Markierungen und -Messungen |
| User-Declared Canonical | Vom Nutzer angegebenes Canonical |
| Viewport | Viewport |
| WCAG 2.0 A Violations | Verstöße WCAG 2.0 A |
| WCAG 2.0 AA Violations | Verstöße WCAG 2.0 AA |
| WCAG 2.0 AAA Violations | Verstöße WCAG 2.0 AAA |
| WCAG 2.1 AA Violations | Verstöße WCAG 2.1 AA |
| WCAG 2.2 AA Violations | Verstöße WCAG 2.2 AA |
| Warnings | Warnungen |
| Word Count | Wortanzahl |
| Word Count Change | Änderung Wortanzahl |
| X-Robots-Tag | X-Robots-Tag |
| rel="next" | rel="next" |
| rel="next" 1 | rel="next" 1 |
| rel="prev" | rel="prev" |
| rel="prev" 1 | rel="prev" 1 |

## Ausnahmen

Englisch bleiben (Planer K2-75): **(a)** Eigennamen von Produkten, Diensten und Standards, **(b)** etablierte Kürzel und
Protokollbegriffe einschließlich wörtlicher Direktiven und Header-Namen, **(c)** Werte aus den Daten (Statuszeilen, Header-Namen, Ankertexte,
URLs – nicht einzeln gelistet). **(d)** Wörter, die im Deutschen gleich lauten und gebräuchlich sind.

| Begriff | Regel | Begründung |
|---|---|---|
| `Crawl` | (d) | im Deutschen gleichlautend und gebräuchlich (Seite „Spider: Crawl“ der Konfiguration) |
| `Profile` | (d) | im Deutschen gleichlautend (Plural von „Profil“, Konfigurationsseite) |
| `Chrome` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Lighthouse` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `PageSpeed` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `PageSpeed Insights` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Search Console` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Google Analytics` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `GA4` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Google Drive` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Looker Studio` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Majestic` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Ahrefs` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Moz` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `JavaScript` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Chromium` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `AMP` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Hreflang` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `SERP` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `MCP` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `WCAG` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `ARIA` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Open Graph` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `JSON-LD` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Microdata` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `RDFa` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Schema.org` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `CrUX` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `OpenSpider` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `URL` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `URLs` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `HTTP` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `HTTPS` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `HTML` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `CSS` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `XML` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `PDF` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `H1` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `H2` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `CSV` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `XLSX` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `JSON` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `API` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `DOM` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `ID` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `Canonical` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `Canonicals` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `noindex` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `nofollow` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `X-Robots-Tag` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `Meta Robots` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `robots.txt` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `Analytics` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `CTR` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `CrUX Cumulative Layout Shift` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `CrUX Interaction to Next Paint (ms)` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `CrUX Origin Cumulative Layout Shift` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `CrUX Origin Interaction to Next Paint (ms)` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `CrUX Origin Time to First Byte (ms)` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `CrUX Time to First Byte (ms)` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Cumulative Layout Shift` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Flash` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Follow` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `H1-1` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `H1-2` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `H2-1` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `H2-2` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `Index` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `Last-Modified` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `Links` | (d) | im Deutschen gleichlautend und gebräuchlich |
| `Max-Image-Preview` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `Max-Snippet` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `Max-Video-Preview` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `NoArchive` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `NoImageIndex` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `NoSnippet` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `NoTranslate` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `Nofollow` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `Noindex` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `None` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `Plugins` | (d) | im Deutschen gleichlautend und gebräuchlich |
| `Position` | (d) | im Deutschen gleichlautend und gebräuchlich |
| `Refresh` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `Rich Results` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Sitemaps` | (d) | im Deutschen gleichlautend und gebräuchlich |
| `Status` | (d) | im Deutschen gleichlautend und gebräuchlich |
| `Total Blocking Time (ms)` | (a) | Eigenname eines Produkts, Dienstes oder Standards |
| `Unavailable_After` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `Viewport` | (d) | im Deutschen gleichlautend und gebräuchlich |
| `rel="next"` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `rel="next" 1` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `rel="prev"` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
| `rel="prev" 1` | (b) | etabliertes Kürzel bzw. wörtlicher Protokoll-/Direktivenbegriff |
