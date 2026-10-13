# 01 – Crawl-Engine & Konfiguration (CRAWL)

Clean-Room-Spezifikation für OpenSpider. Grundlage: öffentliche Benutzerdokumentation, FAQ, Tutorials und Release-Notes (Versionen 18.0 bis 24.3) des Referenzprodukts, in eigenen Worten zusammengefasst. Es wurde weder Code noch Bytecode des Referenzprodukts eingesehen.

Stand: 2026-10-13 (Revision 2 nach Review) · Referenzversion: 24.3 · ID-Präfix: `CRAWL` · Zuständige Crates laut `docs/ARCHITECTURE.md`: `os-engine`, `os-fetch`, `os-url`, `os-render`, `os-store`, `os-cli`.

**Revision 2:** Einträge CRAWL-138 bis CRAWL-144 ergänzt (HTML-Validierung, Green Hosting, Voreinstellungen/Grenzwerte, automatische Crawl-Analyse, Update-Prüfung/Fokus-Modus als Verweis, Kontextmenü der Crawl-Verwaltung, Vergleichs-Detailansichten). Korrigiert: Geltungsbereich der URL-Umschreibung (J), Include/Exclude-Abgleichsemantik (CRAWL-109/110), CLI-Syntax (CRAWL-036/135), Vergleich ohne Speichermodus-Bedingung (CRAWL-032), Kennzeichnung nicht belegter Defaults (CRAWL-018/047/062/065/100), Proxy-Abweichung (CRAWL-122). Bestehende IDs unverändert.

## Konventionen dieses Dokuments

- **Priorität:** P0 = MVP-Kern (ohne das ist OpenSpider kein Crawler), P1 = für Funktionsparität nötig, P2 = später/Nische.
- **„Referenz-Default“** heißt: Wert ist in der öffentlichen Doku ausdrücklich genannt. **„Referenz-Default (abgeleitet)“** heißt: die Doku nennt den Wert nicht wörtlich, er ergibt sich aber eindeutig aus der Beschreibung (z. B. „kann abgeschaltet werden“ ⇒ Standard an). **„OS-Default“** bzw. **„OS-Festlegung“** heißt: Verhalten oder Wert ist dort nicht dokumentiert, OpenSpider legt es selbst fest. Diese Stellen sind in [Abschnitt Q](#q-offene-punkte--zu-verifizieren) gesammelt und per Black-Box-Vergleich (normale Bedienung der Referenz-UI gegen dieselbe Fixture, kein Dekompilieren) zu bestätigen oder zu korrigieren.
- **Kein Lizenz-Gating:** Das Referenzprodukt sperrt einige Funktionen (Speichern/Öffnen, Profile, Formular-Login, JS-Rendering, Vergleich, > 500 URLs) hinter einer Lizenz. In OpenSpider ist alles immer verfügbar.
- **Speicherarchitektur:** Gemäß `docs/ARCHITECTURE.md` ist jeder Crawl eine SQLite-Datei (`*.ospider`). Funktionen, die im Referenzprodukt nur im „Datenbankmodus“ verfügbar sind (Vergleich, Segmente, Auto-Save, Crawl-Verwaltung), sind in OpenSpider immer verfügbar – es gibt keinen Modus, in dem sie deaktiviert sind. Der Referenz-„Arbeitsspeichermodus“ wird ausschließlich als Leistungsprofil (CRAWL-124) abgebildet und schränkt keine Funktion ein.
- **Regex-Abgleichsemantik:** Include, Exclude (CRAWL-109/110) und die URL-Pfad-Limits (CRAWL-076) prüfen **Vollübereinstimmung** (das Muster muss die gesamte URL abdecken, intern implizit `^(?:…)$`). Regex-Ersetzung (CRAWL-104), Vergleichs-URL-Zuordnung (CRAWL-034) und der Segment-Operator „Regex passt“ (CRAWL-134) arbeiten mit **Suche** (Treffer irgendwo in der Zeichenkette). Siehe Q25.
- **Statuscode 0:** URLs ohne HTTP-Antwort (blockiert, DNS-Fehler, Timeout …) erhalten den Statuscode `0`; die Ursache steht in der Spalte `Status`.
- **Spaltennamen:** deutsche UI-Bezeichner; der stabile Export-/Datenmodell-Schlüssel steht beim ersten Vorkommen in `code`-Schrift dahinter.
- **Konfigurationsschlüssel:** Jede Option hat einen stabilen Schlüssel im Konfigurationsprofil (CRAWL-132), Schema `bereich.option` (z. B. `limits.max_depth`). Wo er nicht explizit genannt ist, leitet die Implementierung ihn aus Abschnitt und Optionsname ab und dokumentiert ihn im Profil-Schema.
- **Regex-Dialekt:** Überall, wo Regex verwendet wird (Include/Exclude, Umschreibung, URL-Zuordnung, Segmente), gilt ein einheitlicher Dialekt mit Lookahead/Lookbehind, Rückreferenzen im Ersatztext (`$1`), Inline-Flags `(?i)` und nicht-gierigen Quantoren. Die Rust-Crate `regex` deckt Lookaround nicht ab; `os-url` braucht daher `fancy-regex` (MIT) oder Gleichwertiges. Mit dem Orchestrator abzustimmen (Abhängigkeit in der Architektur bisher `regex`).

### Testfixture-Konvention

Fixtures liegen unter `tests/fixtures/crawl-<name>/` und werden vom Crate `os-testsite` (axum) ausgeliefert. Anforderungen an `os-testsite`, die sich aus diesem Dokument ergeben:
- lauscht auf `127.0.0.1` und `::1` auf zufälligem Port;
- bedient beliebig viele virtuelle Hosts unter der TLD `.test` (z. B. `site.test`, `www.site.test`, `blog.site.test`, `other.test`); die Engine löst sie im Test über die Host-Überschreibung (CRAWL-123) auf, echte DNS-Einträge sind nicht nötig;
- liest je Fixture eine `routes.toml`, die pro Host+Pfad festlegt: Statuscode, Header, Body-Datei, Verzögerung (`delay_ms`), Fehlerverhalten (`reset_mid_body`, `hang`, `fail_first_n`), Auth-Anforderung (Basic/Digest/NTLM/Formular-Session), optional TLS-Zertifikat je Host;
- führt ein Request-Log (Methode, Host, Pfad, Header, Zeitstempel, Verbindungs-ID), das Tests abfragen („wurde `/privat/` angefragt?“, „welcher `User-Agent` kam an?“, „wie viele TCP-Verbindungen?“);
- jede Fixture enthält eine `expected.json` (erwartete URL-Liste mit Adresse, Statuscode, Status, Tiefe, Weiterleitungsziel, intern/extern) als Golden-Output.

Jede Funktion unten nennt den Fixture-Namen und die Routen, die das Verhalten provozieren. Mehrere Funktionen teilen sich Fixtures.

---

## Inhaltsverzeichnis

**A. Crawl-Kern**
- [CRAWL-001 Crawl starten und Start-URL prüfen](#crawl-001-crawl-starten-und-start-url-prüfen) · P0
- [CRAWL-002 URL-Parsing und Auflösung relativer Links](#crawl-002-url-parsing-und-auflösung-relativer-links) · P0
- [CRAWL-003 URL-Normalisierung und Deduplizierung](#crawl-003-url-normalisierung-und-deduplizierung) · P0
- [CRAWL-004 Fragment-Bezeichner](#crawl-004-fragment-bezeichner) · P1
- [CRAWL-005 Crawl-Reihenfolge (Frontier) und Crawl-Tiefe](#crawl-005-crawl-reihenfolge-frontier-und-crawl-tiefe) · P0
- [CRAWL-006 Klassifikation intern/extern](#crawl-006-klassifikation-internextern) · P0
- [CRAWL-007 Crawl-Bereich: Subdomain (Standard)](#crawl-007-crawl-bereich-subdomain-standard) · P0
- [CRAWL-008 Crawl-Bereich: Alle Subdomains](#crawl-008-crawl-bereich-alle-subdomains) · P0
- [CRAWL-009 Crawl-Bereich: Unterordner](#crawl-009-crawl-bereich-unterordner) · P0
- [CRAWL-010 Links außerhalb des Startordners prüfen](#crawl-010-links-außerhalb-des-startordners-prüfen) · P1
- [CRAWL-011 HTTP-Abruf (Methode, Kompression, Header)](#crawl-011-http-abruf-methode-kompression-header) · P0
- [CRAWL-012 Statuscode-Erfassung und Klassen](#crawl-012-statuscode-erfassung-und-klassen) · P0
- [CRAWL-013 Klassifikation „Keine Antwort“](#crawl-013-klassifikation-keine-antwort) · P0
- [CRAWL-014 HTTP-Weiterleitungen (3xx)](#crawl-014-http-weiterleitungen-3xx) · P0
- [CRAWL-015 Meta-Refresh- und HTTP-Refresh-Weiterleitungen](#crawl-015-meta-refresh--und-http-refresh-weiterleitungen) · P0
- [CRAWL-016 JavaScript-Weiterleitungen](#crawl-016-javascript-weiterleitungen) · P1
- [CRAWL-017 Weiterleitungsketten und -schleifen](#crawl-017-weiterleitungsketten-und--schleifen) · P0
- [CRAWL-018 HSTS-Richtlinie respektieren](#crawl-018-hsts-richtlinie-respektieren) · P1
- [CRAWL-019 Inhaltstyp-Erkennung](#crawl-019-inhaltstyp-erkennung) · P0
- [CRAWL-020 „Seiten als HTML annehmen“](#crawl-020-seiten-als-html-annehmen) · P1
- [CRAWL-021 Zeichensatz-Erkennung](#crawl-021-zeichensatz-erkennung) · P0
- [CRAWL-022 Antwortzeit, Größe, Last-Modified](#crawl-022-antwortzeit-größe-last-modified) · P0
- [CRAWL-023 TLS und Zertifikate](#crawl-023-tls-und-zertifikate) · P1
- [CRAWL-024 HTTP-Versionen (HTTP/1.1, HTTP/2)](#crawl-024-http-versionen-http11-http2) · P1/P2
- [CRAWL-025 IPv6](#crawl-025-ipv6) · P2

**B. Crawl-Modi**
- [CRAWL-026 Spider-Modus](#crawl-026-spider-modus) · P0
- [CRAWL-027 Listen-Modus: Eingabequellen](#crawl-027-listen-modus-eingabequellen) · P0
- [CRAWL-028 Listen-Modus: Verhalten](#crawl-028-listen-modus-verhalten) · P0
- [CRAWL-029 Listen-Modus: Export in Upload-Reihenfolge](#crawl-029-listen-modus-export-in-upload-reihenfolge) · P1
- [CRAWL-030 Weiterleitungen/Canonicals immer bis zum Ziel folgen](#crawl-030-weiterleitungencanonicals-immer-bis-zum-ziel-folgen) · P1
- [CRAWL-031 SERP-Modus](#crawl-031-serp-modus) · P1
- [CRAWL-032 Vergleichsmodus: Auswahl und Kategorien](#crawl-032-vergleichsmodus-auswahl-und-kategorien) · P1
- [CRAWL-033 Vergleichsmodus: Änderungserkennung](#crawl-033-vergleichsmodus-änderungserkennung) · P1
- [CRAWL-034 Vergleichsmodus: URL-Zuordnung](#crawl-034-vergleichsmodus-url-zuordnung) · P1
- [CRAWL-035 Vergleichsmodus: Seitenstruktur-Ansicht](#crawl-035-vergleichsmodus-seitenstruktur-ansicht) · P2
- [CRAWL-036 Vergleich automatisch (geplante/CLI-Crawls)](#crawl-036-vergleich-automatisch-geplantecli-crawls) · P2
- [CRAWL-037 API-Modus (Daten ohne Crawl)](#crawl-037-api-modus-daten-ohne-crawl) · P2

**C. Spider-Konfiguration: Ressourcen- und Seitenlinks**
- [CRAWL-038 Grundprinzip „Speichern“ vs. „Crawlen“](#crawl-038-grundprinzip-speichern-vs-crawlen) · P0
- [CRAWL-039 Bilder](#crawl-039-bilder) · P0
- [CRAWL-040 CSS](#crawl-040-css) · P0
- [CRAWL-041 JavaScript-Dateien](#crawl-041-javascript-dateien) · P0
- [CRAWL-042 Medien (Audio/Video)](#crawl-042-medien-audiovideo) · P1
- [CRAWL-043 SWF](#crawl-043-swf) · P2
- [CRAWL-044 Interne Hyperlinks](#crawl-044-interne-hyperlinks) · P0
- [CRAWL-045 Externe Links](#crawl-045-externe-links) · P0
- [CRAWL-046 Canonicals](#crawl-046-canonicals) · P0
- [CRAWL-047 Paginierung (rel next/prev)](#crawl-047-paginierung-rel-nextprev) · P1
- [CRAWL-048 Hreflang](#crawl-048-hreflang) · P1
- [CRAWL-049 AMP](#crawl-049-amp) · P2
- [CRAWL-050 Meta Refresh und iframes](#crawl-050-meta-refresh-und-iframes) · P1
- [CRAWL-051 Mobile-Alternate-Links](#crawl-051-mobile-alternate-links) · P2
- [CRAWL-052 Link-Quellen im HTML (welche Elemente Links erzeugen)](#crawl-052-link-quellen-im-html-welche-elemente-links-erzeugen) · P0
- [CRAWL-053 Nicht crawlbare Links speichern](#crawl-053-nicht-crawlbare-links-speichern) · P1
- [CRAWL-054 Ungültige Links crawlen](#crawl-054-ungültige-links-crawlen) · P2
- [CRAWL-055 Bilder aus srcset extrahieren](#crawl-055-bilder-aus-srcset-extrahieren) · P1
- [CRAWL-056 Nofollow-Links folgen (intern/extern)](#crawl-056-nofollow-links-folgen-internextern) · P0
- [CRAWL-057 Links in PDF-Dokumenten](#crawl-057-links-in-pdf-dokumenten) · P1

**D. XML-Sitemaps**
- [CRAWL-058 Sitemap-Erkennung über robots.txt](#crawl-058-sitemap-erkennung-über-robotstxt) · P1
- [CRAWL-059 Verlinkte und vorgegebene Sitemaps crawlen](#crawl-059-verlinkte-und-vorgegebene-sitemaps-crawlen) · P1
- [CRAWL-060 Sitemap-Parser](#crawl-060-sitemap-parser) · P1

**E. Extraktion (crawl-seitige Schalter)**
- [CRAWL-061 Schalter für Seitendetails](#crawl-061-schalter-für-seitendetails) · P1
- [CRAWL-062 URL-Details: Antwortzeit, Last-Modified, HTTP-Header, Cookies](#crawl-062-url-details-antwortzeit-last-modified-http-header-cookies) · P1
- [CRAWL-063 HTML und gerendertes HTML speichern](#crawl-063-html-und-gerendertes-html-speichern) · P1
- [CRAWL-064 PDF speichern und Eigenschaften extrahieren](#crawl-064-pdf-speichern-und-eigenschaften-extrahieren) · P2
- [CRAWL-065 Direktiven- und Strukturdaten-Schalter](#crawl-065-direktiven--und-strukturdaten-schalter) · P1

**F. Limits**
- [CRAWL-066 Maximale Gesamtzahl URLs](#crawl-066-maximale-gesamtzahl-urls) · P0
- [CRAWL-067 Maximale Crawl-Tiefe](#crawl-067-maximale-crawl-tiefe) · P0
- [CRAWL-068 URLs pro Crawl-Tiefe](#crawl-068-urls-pro-crawl-tiefe) · P2
- [CRAWL-069 Maximale Ordnertiefe](#crawl-069-maximale-ordnertiefe) · P1
- [CRAWL-070 Maximale Anzahl Query-Parameter](#crawl-070-maximale-anzahl-query-parameter) · P1
- [CRAWL-071 Maximale URLs pro Subdomain](#crawl-071-maximale-urls-pro-subdomain) · P2
- [CRAWL-072 Maximale Anzahl verfolgter Weiterleitungen](#crawl-072-maximale-anzahl-verfolgter-weiterleitungen) · P0
- [CRAWL-073 Maximale URL-Länge](#crawl-073-maximale-url-länge) · P1
- [CRAWL-074 Maximale Links pro URL](#crawl-074-maximale-links-pro-url) · P1
- [CRAWL-075 Maximale Seitengröße](#crawl-075-maximale-seitengröße) · P1
- [CRAWL-076 Limit nach URL-Pfad](#crawl-076-limit-nach-url-pfad) · P2

**G. Rendering**
- [CRAWL-077 Rendering-Modus wählen](#crawl-077-rendering-modus-wählen) · P0
- [CRAWL-078 JavaScript-Rendering mit Headless-Chromium](#crawl-078-javascript-rendering-mit-headless-chromium) · P1
- [CRAWL-079 Link-Herkunft (Roh-HTML vs. gerendertes HTML)](#crawl-079-link-herkunft-roh-html-vs-gerendertes-html) · P1
- [CRAWL-080 AJAX-Timeout](#crawl-080-ajax-timeout) · P1
- [CRAWL-081 Viewport und Geräte-Voreinstellungen](#crawl-081-viewport-und-geräte-voreinstellungen) · P1
- [CRAWL-082 Gerenderte Screenshots](#crawl-082-gerenderte-screenshots) · P1
- [CRAWL-083 Blockierte Ressourcen beim Rendern](#crawl-083-blockierte-ressourcen-beim-rendern) · P1
- [CRAWL-084 JavaScript-Fehler / Konsolen-Log](#crawl-084-javascript-fehler--konsolen-log) · P2
- [CRAWL-085 Shadow DOM und iframes abflachen](#crawl-085-shadow-dom-und-iframes-abflachen) · P2
- [CRAWL-086 Website-Archiv (Ordner / WARC)](#crawl-086-website-archiv-ordner--warc) · P2
- [CRAWL-087 Altes AJAX-Crawling-Schema](#crawl-087-altes-ajax-crawling-schema) · P2

**H. Erweitert**
- [CRAWL-088 Cookie-Verhalten](#crawl-088-cookie-verhalten) · P0
- [CRAWL-089 Nicht indexierbare URLs bei Problemen ignorieren](#crawl-089-nicht-indexierbare-urls-bei-problemen-ignorieren) · P1
- [CRAWL-090 Paginierte URLs bei Duplikat-Filtern ignorieren](#crawl-090-paginierte-urls-bei-duplikat-filtern-ignorieren) · P2
- [CRAWL-091 noindex respektieren](#crawl-091-noindex-respektieren) · P1
- [CRAWL-092 Canonical respektieren](#crawl-092-canonical-respektieren) · P1
- [CRAWL-093 next/prev respektieren](#crawl-093-nextprev-respektieren) · P2
- [CRAWL-094 Selbstreferenzierenden Meta-Refresh respektieren](#crawl-094-selbstreferenzierenden-meta-refresh-respektieren) · P2
- [CRAWL-095 Antwort-Timeout](#crawl-095-antwort-timeout) · P0
- [CRAWL-096 Wiederholung bei 5xx-Antworten](#crawl-096-wiederholung-bei-5xx-antworten) · P1
- [CRAWL-097 Wiederholung bei Netzwerkfehlern und 429](#crawl-097-wiederholung-bei-netzwerkfehlern-und-429) · P1

**I. robots.txt**
- [CRAWL-098 robots.txt abrufen und auswerten](#crawl-098-robotstxt-abrufen-und-auswerten) · P0
- [CRAWL-099 robots.txt-Modi](#crawl-099-robotstxt-modi) · P0
- [CRAWL-100 Fehlerfälle beim robots.txt-Abruf](#crawl-100-fehlerfälle-beim-robotstxt-abruf) · P0
- [CRAWL-101 Anzeige blockierter URLs (intern/extern)](#crawl-101-anzeige-blockierter-urls-internextern) · P1
- [CRAWL-102 Eigene robots.txt pro Subdomain und Tester](#crawl-102-eigene-robotstxt-pro-subdomain-und-tester) · P1

**J. URL-Umschreibung**
- [CRAWL-103 Parameter entfernen](#crawl-103-parameter-entfernen) · P1
- [CRAWL-104 Regex-Ersetzung](#crawl-104-regex-ersetzung) · P1
- [CRAWL-105 Gefundene URLs kleinschreiben](#crawl-105-gefundene-urls-kleinschreiben) · P2
- [CRAWL-106 Prozent-Kodierungsmodus](#crawl-106-prozent-kodierungsmodus) · P2
- [CRAWL-107 Testfeld für Umschreibungen](#crawl-107-testfeld-für-umschreibungen) · P1

**K. Bereichssteuerung**
- [CRAWL-108 CDNs als intern behandeln](#crawl-108-cdns-als-intern-behandeln) · P1
- [CRAWL-109 Include-Regeln (Regex)](#crawl-109-include-regeln-regex) · P0
- [CRAWL-110 Exclude-Regeln (Regex)](#crawl-110-exclude-regeln-regex) · P0
- [CRAWL-111 Regex-Tester für Include/Exclude](#crawl-111-regex-tester-für-includeexclude) · P1

**L. Geschwindigkeit, User-Agent, Header**
- [CRAWL-112 Parallele Threads](#crawl-112-parallele-threads) · P0
- [CRAWL-113 Maximale URLs pro Sekunde](#crawl-113-maximale-urls-pro-sekunde) · P0
- [CRAWL-114 User-Agent und Voreinstellungen](#crawl-114-user-agent-und-voreinstellungen) · P0
- [CRAWL-115 Eigene HTTP-Request-Header](#crawl-115-eigene-http-request-header) · P1
- [CRAWL-116 Accept-Language](#crawl-116-accept-language) · P1

**M. Authentifizierung und Proxy**
- [CRAWL-117 Standard-Authentifizierung: Basic](#crawl-117-standard-authentifizierung-basic) · P0
- [CRAWL-118 Standard-Authentifizierung: Digest](#crawl-118-standard-authentifizierung-digest) · P1
- [CRAWL-119 NTLM und Negotiate/Kerberos](#crawl-119-ntlm-und-negotiatekerberos) · P2
- [CRAWL-120 Formularbasierte Anmeldung (eingebetteter Browser)](#crawl-120-formularbasierte-anmeldung-eingebetteter-browser) · P1
- [CRAWL-121 Authentifizierungsprofile (verschlüsselt)](#crawl-121-authentifizierungsprofile-verschlüsselt) · P2
- [CRAWL-122 Proxy](#crawl-122-proxy) · P1
- [CRAWL-123 Host-Auflösung überschreiben](#crawl-123-host-auflösung-überschreiben) · P1

**N. Speicher, Persistenz, Steuerung**
- [CRAWL-124 Speichermodus: Datenbank vs. Arbeitsspeicher](#crawl-124-speichermodus-datenbank-vs-arbeitsspeicher) · P0
- [CRAWL-125 Automatisches Speichern und Wiederherstellung](#crawl-125-automatisches-speichern-und-wiederherstellung) · P0
- [CRAWL-126 Crawl pausieren und fortsetzen](#crawl-126-crawl-pausieren-und-fortsetzen) · P0
- [CRAWL-127 Crawl speichern, öffnen, importieren, exportieren](#crawl-127-crawl-speichern-öffnen-importieren-exportieren) · P0
- [CRAWL-128 Crawl-Verwaltung (Projekte, Sperren, Aufbewahrung)](#crawl-128-crawl-verwaltung-projekte-sperren-aufbewahrung) · P1
- [CRAWL-129 Speicherwarnung und Ressourcenschutz](#crawl-129-speicherwarnung-und-ressourcenschutz) · P1
- [CRAWL-130 Fortschritt und Crawl-Statistik](#crawl-130-fortschritt-und-crawl-statistik) · P0
- [CRAWL-131 Einzelne URLs neu crawlen, entfernen, hinzufügen](#crawl-131-einzelne-urls-neu-crawlen-entfernen-hinzufügen) · P1
- [CRAWL-132 Konfigurationsprofile](#crawl-132-konfigurationsprofile) · P0
- [CRAWL-133 Konfigurations-Diff zum Standard](#crawl-133-konfigurations-diff-zum-standard) · P2
- [CRAWL-134 Segmente](#crawl-134-segmente) · P1
- [CRAWL-135 Kommandozeile / Headless-Betrieb](#crawl-135-kommandozeile--headless-betrieb) · P1
- [CRAWL-136 Geplante Crawls](#crawl-136-geplante-crawls) · P2
- [CRAWL-137 Oberflächensprache](#crawl-137-oberflächensprache) · P2

**O. Ergänzungen (Revision 2)**
- [CRAWL-138 HTML-Validierung durchführen](#crawl-138-html-validierung-durchführen) · P1
- [CRAWL-139 Green Hosting für die CO2-Berechnung](#crawl-139-green-hosting-für-die-co2-berechnung) · P2
- [CRAWL-140 Voreinstellungen (crawl-relevante Grenzwerte)](#crawl-140-voreinstellungen-crawl-relevante-grenzwerte) · P0/P1
- [CRAWL-141 Automatische Crawl-Analyse am Crawl-Ende](#crawl-141-automatische-crawl-analyse-am-crawl-ende) · P0
- [CRAWL-142 Update-Prüfung und Fokus-Modus (Abgrenzung)](#crawl-142-update-prüfung-und-fokus-modus-abgrenzung) · P2
- [CRAWL-143 Kontextmenü der Crawl-Verwaltung](#crawl-143-kontextmenü-der-crawl-verwaltung) · P1
- [CRAWL-144 Vergleichsmodus: Detailansichten (Links, Issues, gerenderte Seite)](#crawl-144-vergleichsmodus-detailansichten-links-issues-gerenderte-seite) · P1/P2

**Q.** [Offene Punkte / zu verifizieren](#q-offene-punkte--zu-verifizieren)

**R.** [Quellen](#r-quellen)

---
## A. Crawl-Kern

### CRAWL-001 Crawl starten und Start-URL prüfen
- **Priorität:** P0
- **Beschreibung:** Der Nutzer gibt eine Start-URL ein und startet den Crawl. Die Engine bereinigt die Eingabe, ergänzt fehlende Schemata, prüft die Syntax und legt die URL als Tiefe 0 in die Warteschlange. Die URL-Leiste bietet eine Autovervollständigung aus zuvor gecrawlten Adressen.
- **Eingaben/Konfiguration:** URL-Feld; Schalter „Start“; Crawl-Bereich (CRAWL-007/008/009); aktive Konfiguration.
- **Ausgabe:** Start-URL erscheint als erste Zeile im Tab „Intern“ mit `Crawl-Tiefe` `crawl_depth` = 0.
- **Akzeptanzkriterien:**
  - [ ] Führende/abschließende Leerzeichen werden entfernt.
  - [ ] Eingabe ohne Schema (`site.test`) wird zu `http://site.test/` ergänzt; folgt eine Weiterleitung auf HTTPS, wird diese normal verarbeitet (CRAWL-014).
  - [ ] Eingabe ohne Pfad erhält den Pfad `/`.
  - [ ] Großbuchstaben im Schema/Host werden kleingeschrieben (`HTTPS://Site.TEST/a` → `https://site.test/a`); der Pfad bleibt unverändert.
  - [ ] Ungültige Eingaben (`htp:/x`, leerer Host, Leerzeichen im Host, nicht unterstütztes Schema wie `ftp:`) werden vor dem Start mit einer Fehlermeldung abgewiesen; es wird kein Crawl angelegt.
  - [ ] Internationalisierte Domainnamen (`müller.test`) werden per IDNA/Punycode aufgelöst, in der Oberfläche aber in Unicode angezeigt.
  - [ ] Ein laufender Crawl muss pausiert oder gelöscht werden, bevor eine neue Start-URL gestartet werden kann; der vorherige Crawl ist dabei bereits in seiner Crawl-Datei gesichert (CRAWL-125) und bleibt in der Crawl-Verwaltung verfügbar.
  - [ ] Die Start-URL wird unabhängig von Include-/Exclude-Regeln gecrawlt (CRAWL-109/110).
  - [ ] Die zuletzt verwendeten 50 Start-URLs werden als Autovervollständigung angeboten (OS-Default).
- **Testfixture:** `crawl-start-url`: `site.test/` (200, 3 Links), `http://site.test/` → 301 auf `https://site.test/`; Testfälle für Eingaben `site.test`, `  HTTPS://SITE.test  `, `htp:/x`, `müller.test`.

### CRAWL-002 URL-Parsing und Auflösung relativer Links
- **Priorität:** P0
- **Beschreibung:** Gefundene Link-Ziele werden nach WHATWG-URL-Standard gegen die Basis-URL aufgelöst. Die Basis ist das erste gültige `<base href>` im Dokument, sonst die finale URL nach Weiterleitungen.
- **Eingaben/Konfiguration:** keine (immer aktiv).
- **Ausgabe:** absolute URL in Spalte `Adresse` `address`; die Originalschreibweise steht im Outlinks-Detail als `Link-Pfad` (OS-Erweiterung, nur bei aktivierter Speicherung).
- **Akzeptanzkriterien:**
  - [ ] Relative Pfade (`a.html`, `../b/`, `./c`, `/d`), protokollrelative URLs (`//cdn.test/x.js`) und reine Query-Links (`?p=2`) werden korrekt aufgelöst.
  - [ ] `<base href>` wird berücksichtigt; ein relatives `base href` wird selbst gegen die Dokument-URL aufgelöst; ab dem zweiten `<base>`-Element wird ignoriert.
  - [ ] Ein `<base>` außerhalb von `<head>` wird trotzdem ausgewertet (Browser-Verhalten).
  - [ ] HTML-Entities im Attribut (`&amp;`) werden vor dem Auflösen dekodiert.
  - [ ] Führende/abschließende Whitespaces sowie Tabs/Zeilenumbrüche innerhalb des `href` werden wie im Browser entfernt.
  - [ ] Pfadsegmente `.` und `..` werden aufgelöst; `..` über die Wurzel hinaus bleibt bei `/`.
  - [ ] Links mit Schemata `mailto:`, `tel:`, `javascript:`, `data:`, `about:`, `file:` werden nicht gecrawlt; `javascript:`-Links werden als nicht crawlbar gezählt (CRAWL-053).
  - [ ] Leere `href=""` verweist auf die aktuelle Seite (Selbstlink) und wird nicht erneut gecrawlt.
- **Testfixture:** `crawl-url-resolve`: `/dir/sub/page.html` mit allen genannten Link-Formen, `<base href="/other/">` auf `/based.html`, zweites `<base>`, `href=" /spaces "`, `href="a&amp;b"`.

### CRAWL-003 URL-Normalisierung und Deduplizierung
- **Priorität:** P0
- **Beschreibung:** Bevor eine URL in die Warteschlange kommt, wird sie in eine kanonische Schlüsselform gebracht. Zwei URLs mit gleichem Schlüssel gelten als dieselbe URL und werden nur einmal abgerufen. Die Normalisierung ist bewusst konservativ: Sie ändert nichts, was ein Server anders interpretieren könnte.
- **Eingaben/Konfiguration:** URL-Umschreibung (CRAWL-103–106) wird vor der Normalisierung angewendet.
- **Ausgabe:** Spalte `Adresse` `address` zeigt die normalisierte URL in lesbarer Form (Nicht-ASCII-Zeichen und IDN dekodiert); Spalte `URL-kodierte Adresse` `url_encoded_address` (Referenz-Spalte des Tabs „Intern“) zeigt dieselbe URL exakt so, wie sie im Request gesendet wurde (Punycode-Host, prozentkodierter Pfad/Query).
- **Akzeptanzkriterien:**
  - [ ] `/café` erscheint in `Adresse` als `https://site.test/café` und in `URL-kodierte Adresse` als `https://site.test/caf%C3%A9`; `müller.test` erscheint in `URL-kodierte Adresse` als `xn--mller-kva.test`.
  - [ ] Für reine ASCII-URLs ohne zu kodierende Zeichen sind `Adresse` und `URL-kodierte Adresse` byte-identisch.
  - [ ] Schema und Host werden kleingeschrieben; Pfad und Query bleiben in der Groß-/Kleinschreibung erhalten (`/Page` ≠ `/page`).
  - [ ] Standardports werden entfernt (`:80` bei http, `:443` bei https); andere Ports bleiben Bestandteil der URL.
  - [ ] Ein abschließender Punkt im Hostnamen (`site.test.`) wird entfernt.
  - [ ] Nicht-ASCII-Zeichen und unzulässige Zeichen in Pfad/Query werden prozentkodiert (Großbuchstaben-Hex als Standard, CRAWL-106); bereits kodierte Sequenzen werden nicht doppelt kodiert.
  - [ ] Prozentkodierte unreservierte Zeichen (`%41` → `A`, `%7E` → `~`) werden dekodiert.
  - [ ] Reihenfolge der Query-Parameter bleibt unverändert (`?a=1&b=2` ≠ `?b=2&a=1`).
  - [ ] Ein leerer Query-String (`/x?`) bleibt als eigene URL erhalten und unterscheidet sich von `/x` (OS-Default, siehe Q).
  - [ ] `http://` und `https://` derselben Seite sind zwei verschiedene URLs.
  - [ ] `/ordner` und `/ordner/` sind zwei verschiedene URLs.
  - [ ] Fragment wird standardmäßig entfernt (CRAWL-004).
  - [ ] Eine URL wird nie zweimal abgerufen, auch nicht, wenn sie von mehreren Threads gleichzeitig entdeckt wird (Test mit 50 Seiten, die alle auf dieselbe URL verlinken, 20 Threads: genau 1 Request im Request-Log).
- **Testfixture:** `crawl-normalize`: Seite mit Links auf `HTTP://SITE.TEST:80/a`, `/a`, `/%7Euser`, `/~user`, `/A`, `/a?`, `/a?b=2&a=1`, `/a?a=1&b=2`, `/ordner`, `/ordner/`, `/caf%C3%A9`, `/café`.

### CRAWL-004 Fragment-Bezeichner
- **Priorität:** P1
- **Beschreibung:** Standardmäßig wird der Teil ab `#` ignoriert. Optional können Fragment-URLs als eigenständige URLs behandelt werden (für Single-Page-Apps mit Hash-Routing).
- **Eingaben/Konfiguration:** Erweitert › „Fragment-Bezeichner crawlen“ (Standard: aus).
- **Ausgabe:** bei aktivierter Option eigene Zeilen je Fragment-URL.
- **Akzeptanzkriterien:**
  - [ ] Option aus: `/a#x`, `/a#y`, `/a` ergeben eine einzige URL `/a`, ein Request.
  - [ ] Option an: drei unterschiedliche Zeilen; jede URL erzeugt einen eigenen Request (mit identischem Pfad, da Fragmente nicht übertragen werden) bzw. bei JS-Rendering einen eigenen Render-Durchlauf mit gesetztem Fragment.
  - [ ] Hashbang-URLs (`#!`) werden nur über CRAWL-087 behandelt, nicht über diese Option.
  - [ ] Ein reiner Sprunglink `#oben` erzeugt bei ausgeschalteter Option keinen Outlink-Eintrag auf eine neue URL, sondern einen Selbstlink.
- **Testfixture:** `crawl-fragments`: `/spa.html` mit Links `#/produkte`, `#/kontakt`, `#oben`; JS-Router, der je nach Hash einen anderen `<title>` setzt.

### CRAWL-005 Crawl-Reihenfolge (Frontier) und Crawl-Tiefe
- **Priorität:** P0
- **Beschreibung:** Der Crawl läuft in Breitensuche: Alle URLs der Tiefe n werden vor denen der Tiefe n+1 eingeplant. Die Crawl-Tiefe einer URL ist die minimale Anzahl Klicks von der Start-URL über gecrawlte Links. Weiterleitungen und Canonicals zählen jeweils als ein Schritt (OS-Festlegung analog Referenzverhalten: Ziel einer Weiterleitung hat Tiefe Quelle + 1).
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** Spalte `Crawl-Tiefe` `crawl_depth`; Wert leer für URLs, die nicht über Links erreicht wurden (z. B. nur über Sitemap, im Listenmodus = 0).
- **Akzeptanzkriterien:**
  - [ ] Bei einer Kette Start → A → B → C hat C Tiefe 3.
  - [ ] Wird C zusätzlich direkt von der Startseite verlinkt, hat C Tiefe 1 (Minimum), auch wenn die Verlinkung über A zuerst entdeckt wurde.
  - [ ] Breitensuche: Im Request-Log erscheint keine URL der Tiefe 2, bevor alle URLs der Tiefe 1 angefragt wurden (bei 1 Thread streng; bei mehreren Threads gilt: kein Tiefe-n+1-Request, solange Tiefe-n-URLs noch nicht gestartet sind).
  - [ ] URLs, die ausschließlich über eine XML-Sitemap gefunden wurden, haben keine Crawl-Tiefe (leer) und werden nach allen per Link erreichbaren URLs gecrawlt.
  - [ ] Ressourcen (Bilder, CSS, JS) erhalten die Tiefe der einbindenden Seite + 1.
- **Testfixture:** `crawl-depth`: lineare Kette `/` → `/1` → `/2` → … → `/10`; zusätzlicher Direktlink `/` → `/7`; `/sitemap.xml` mit `/nur-sitemap`.

### CRAWL-006 Klassifikation intern/extern
- **Priorität:** P0
- **Beschreibung:** Jede URL wird als intern oder extern eingestuft. Intern ist alles, was im aktuellen Crawl-Bereich liegt (CRAWL-007–009) oder als CDN freigegeben ist (CRAWL-108). Alles andere ist extern.
- **Eingaben/Konfiguration:** Crawl-Bereich, CDN-Liste.
- **Ausgabe:** Tabs „Intern“ und „Extern“; jede URL erscheint in genau einem davon.
- **Akzeptanzkriterien:**
  - [ ] Externe URLs werden nur einmal abgerufen (Status prüfen), ihre Links werden nie weiterverfolgt (keine Tiefe > Quelle + 1 auf fremden Hosts).
  - [ ] Externe HTML-Seiten werden nicht nach Links geparst; Titel etc. werden nicht extrahiert (OS-Festlegung, siehe Q).
  - [ ] Ein Wechsel nur des Schemas (`http` ↔ `https`) auf demselben Host bleibt intern.
  - [ ] Ein anderer Port auf demselben Host gilt als intern (OS-Default, siehe Q).
  - [ ] `www.site.test` und `site.test` sind unterschiedliche Subdomains; im Modus „Subdomain“ ist die jeweils andere extern.
- **Testfixture:** `crawl-scope-basic`: `site.test` verlinkt auf `www.site.test/`, `blog.site.test/`, `other.test/`, `https://site.test/`, `site.test:8081/`; `other.test/` enthält weitere Links, die nicht verfolgt werden dürfen.

### CRAWL-007 Crawl-Bereich: Subdomain (Standard)
- **Priorität:** P0
- **Beschreibung:** Standardbereich. Gecrawlt wird nur der exakte Host der Start-URL. Links auf andere Subdomains derselben Domain werden als extern behandelt.
- **Eingaben/Konfiguration:** Bereichsumschalter in der Hauptleiste: „Subdomain“ (Standard) / „Alle Subdomains“ / „Unterordner“. Zusätzlich existiert in der Referenz unter Spider › Crawl ein Kontrollkästchen „Alle Subdomains crawlen“ (Referenz-Default: aus). Beide Bedienorte schreiben in **denselben** Konfigurationsschlüssel: `scope.all_subdomains` (bool). Der Unterordner-Modus ist ein davon unabhängiger Schlüssel `scope.subfolder_only` (bool), sodass die Kombination „Alle Subdomains + Unterordner“ (CRAWL-009) abbildbar ist.
- **Ausgabe:** Tab „Intern“ enthält ausschließlich URLs des Start-Hosts (plus CDNs).
- **Akzeptanzkriterien:**
  - [ ] Wird „Alle Subdomains crawlen“ im Konfigurationsdialog angehakt, zeigt der Bereichsumschalter der Hauptleiste sofort „Alle Subdomains“ – und umgekehrt; im gespeicherten Profil existiert nur `scope.all_subdomains`, kein zweiter Schlüssel.
  - [ ] Ein Profil mit `scope.all_subdomains = true` lädt den Umschalter auf „Alle Subdomains“.
  - [ ] Start `https://www.site.test/`: `https://blog.site.test/` landet im Tab „Extern“ mit Statuscode, wird aber nicht weitergecrawlt.
  - [ ] Ausnahme: Startet man auf der Stammdomain ohne Subdomain (`https://site.test/`), werden im Standardbereich alle Subdomains als intern behandelt (Referenzverhalten). Beim Start auf `www.site.test` gilt das nicht.
- **Testfixture:** `crawl-scope-subdomains`: Hosts `site.test`, `www.site.test`, `blog.site.test`, `shop.site.test` mit gegenseitiger Verlinkung; zwei Testläufe (Start Stammdomain / Start `www`).

### CRAWL-008 Crawl-Bereich: Alle Subdomains
- **Priorität:** P0
- **Beschreibung:** Alle Hosts, die auf dieselbe registrierbare Domain (eTLD+1 nach Public Suffix List) enden, sind intern.
- **Eingaben/Konfiguration:** Bereichsumschalter „Alle Subdomains“.
- **Ausgabe:** Tab „Intern“ enthält alle Subdomains.
- **Akzeptanzkriterien:**
  - [ ] Start `https://www.site.test/` mit „Alle Subdomains“: `blog.site.test`, `shop.site.test`, `a.b.site.test` sind intern und werden vollständig gecrawlt.
  - [ ] Die registrierbare Domain wird über die Public Suffix List bestimmt: Bei `www.example.co.uk` ist `foo.example.co.uk` intern, `other.co.uk` extern. Bei `user.github.io` ist `other.github.io` extern.
  - [ ] Die robots.txt wird pro Subdomain separat abgerufen und angewandt (CRAWL-098).
- **Testfixture:** `crawl-scope-subdomains` (wie oben) + Host `a.b.site.test`; zusätzlich Unit-Tests der eTLD+1-Berechnung mit `co.uk`, `github.io`, `.test`.

### CRAWL-009 Crawl-Bereich: Unterordner
- **Priorität:** P0
- **Beschreibung:** Gecrawlt werden nur URLs, deren Pfad mit dem Ordner der Start-URL beginnt. Ordner ist der Pfad bis einschließlich des letzten `/`.
- **Eingaben/Konfiguration:** Bereichsumschalter „Unterordner“; Option „Außerhalb des Startordners crawlen“ (Standard: aus); kombinierbar mit „Alle Subdomains“.
- **Ausgabe:** Tab „Intern“ enthält Ordner-URLs; URLs außerhalb erscheinen gemäß CRAWL-010.
- **Akzeptanzkriterien:**
  - [ ] Start `https://site.test/blog/`: `/blog/a/` und `/blog/b.html` werden gecrawlt; `/shop/` nicht.
  - [ ] Start `https://site.test/blog` (ohne abschließenden Schrägstrich): Startordner ist `/`; die Oberfläche zeigt einen Hinweis, dass ein Schrägstrich für die Ordnererkennung fehlt.
  - [ ] Die Start-URL muss mit 2xx antworten und auf Seiten im Ordner verlinken; antwortet sie mit 3xx in einen anderen Ordner, wird der Crawl nach dem Abruf des Ziels beendet und ein Hinweis angezeigt.
  - [ ] Mit „Außerhalb des Startordners crawlen“ = an wird die ganze Subdomain gecrawlt, der Start erfolgt aber im Ordner.
  - [ ] Kombination mit „Alle Subdomains“: Ordnerbeschränkung gilt auf jeder Subdomain (OS-Festlegung; siehe Q).
- **Testfixture:** `crawl-scope-folder`: `/blog/` (Links auf `/blog/a/`, `/blog/b.html`, `/shop/`, `/`), `/blog/a/` (Link auf `/blog/a/tief/`), `/blogger/` (darf trotz Präfix-Ähnlichkeit nicht als Teil von `/blog/` gelten).

### CRAWL-010 Links außerhalb des Startordners prüfen
- **Priorität:** P1
- **Beschreibung:** Beim Ordner-Crawl werden Links aus dem Ordner heraus standardmäßig einmal abgerufen (Status), aber nicht weiterverfolgt – analog zu externen Links.
- **Eingaben/Konfiguration:** Spider › Crawl › „Links außerhalb des Startordners prüfen“ (Standard: an).
- **Ausgabe:** Solche URLs erscheinen im Tab „Intern“ mit Statuscode, aber ohne eigene Outlinks.
- **Akzeptanzkriterien:**
  - [ ] Standard: `/shop/` wird angefragt (genau ein Request), die Links auf `/shop/` werden nicht angefragt.
  - [ ] Option aus: `/shop/` wird nicht angefragt und erscheint nicht in den Tabellen; der Link auf `/shop/` erscheint aber in den Outlinks von `/blog/` ohne Statuscode.
  - [ ] Ressourcen (Bilder/CSS/JS) außerhalb des Ordners werden immer geprüft, wenn ihr Ressourcentyp gecrawlt wird.
- **Testfixture:** `crawl-scope-folder` (wie oben), `/shop/` verlinkt auf `/shop/produkt/`.

### CRAWL-011 HTTP-Abruf (Methode, Kompression, Header)
- **Priorität:** P0
- **Beschreibung:** Alle URLs werden per `GET` abgerufen. Die Engine sendet einen festen Satz Standard-Header, dekomprimiert transparent und liest den Body bis zur maximalen Seitengröße (CRAWL-075).
- **Eingaben/Konfiguration:** User-Agent (CRAWL-114), eigene Header (CRAWL-115), Accept-Language (CRAWL-116).
- **Ausgabe:** Spalten `Statuscode`, `Status`, `Inhaltstyp`, `Größe (Bytes)` `size`, `Übertragen (Bytes)` `transferred`, `Gesamt übertragen (Bytes)` `total_transferred` (Definition in CRAWL-022), `HTTP-Version` `http_version` (CRAWL-024), `URL-kodierte Adresse` (CRAWL-003).
- **Akzeptanzkriterien:**
  - [ ] Standardheader: `User-Agent`, `Accept` (HTML bevorzugt, `*/*` als Rückfall), `Accept-Encoding: gzip, deflate, br`, `Connection: keep-alive`; keine Cookies außer gemäß CRAWL-088.
  - [ ] Antworten mit `Content-Encoding` gzip, deflate (zlib und raw) und br werden dekomprimiert; `Größe` ist die dekomprimierte, `Übertragen` die übertragene Byte-Zahl.
  - [ ] `Transfer-Encoding: chunked` wird unterstützt.
  - [ ] Defekte Kompression (falscher Header, kaputte Daten) führt nicht zum Absturz: Status „Verbindungsfehler“ mit Detailtext, Statuscode bleibt der empfangene.
  - [ ] Keep-Alive-Verbindungen werden pro Host wiederverwendet (Test: 20 Seiten auf einem Host erzeugen bei 1 Thread höchstens 2 TCP-Verbindungen).
  - [ ] HEAD-Requests werden nie verwendet (einige Server antworten darauf anders).
- **Testfixture:** `crawl-http-basic`: `/gzip` (gzip), `/br` (Brotli), `/deflate-raw`, `/chunked`, `/broken-gzip` (Header gzip, Body unkomprimiert), `/echo-headers` (gibt empfangene Header als JSON zurück).

### CRAWL-012 Statuscode-Erfassung und Klassen
- **Priorität:** P0
- **Beschreibung:** Für jede URL werden HTTP-Statuscode und Statustext gespeichert und einer Klasse zugeordnet.
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** Spalten `Statuscode` `status_code`, `Status` `status` (Reason-Phrase bzw. Standardtext); Klassen für die Filter des Tabs „Antwortcodes“: Erfolgreich (2xx), Weiterleitung (3xx), Weiterleitung (Meta Refresh), Weiterleitung (HTTP Refresh), Weiterleitung (JavaScript), Clientfehler (4xx), Serverfehler (5xx), Keine Antwort, Durch robots.txt blockiert, Blockierte Ressource, **Sonstige** (OS-Erweiterung, nur Codes außerhalb 100–599; der Filter wird im Tab „Antwortcodes“ nur angezeigt, wenn er mindestens einen Treffer hat). Die Filterdefinitionen des Tabs „Antwortcodes“ selbst gehören zum Bereich Technik (`03-tabs-technical.md`); dieser muss den Filter „Sonstige“ übernehmen.
- **Akzeptanzkriterien:**
  - [ ] Jeder Statuscode 100–599 wird unverändert übernommen, auch nicht registrierte wie 218, 299, 418, 499, 520–530.
  - [ ] Fehlt die Reason-Phrase (HTTP/2 oder leer), wird der Standardtext laut IANA-Registry eingesetzt; für unbekannte Codes bleibt `Status` leer.
  - [ ] 1xx-Zwischenantworten (z. B. 103 Early Hints) werden übersprungen; gespeichert wird die finale Antwort.
  - [ ] Codes außerhalb 100–599 (z. B. 600, 999) werden gespeichert und der Klasse „Sonstige“ zugeordnet (OS-Festlegung; die Referenz kennt diese Klasse nicht). Sie erscheinen im Filter „Sonstige“ und in keinem anderen Statuscode-Filter (insbesondere nicht unter „Serverfehler (5xx)“ oder „Keine Antwort“).
  - [ ] Ohne Codes außerhalb 100–599 ist der Filter „Sonstige“ ausgeblendet; die Summe aller Klassenfilter entspricht der Zeilenzahl des Filters „Alle“.
  - [ ] 304 ohne vorherige bedingte Anfrage wird als 3xx-Klasse geführt, erzeugt aber keine Weiterleitung (kein `Location`).
- **Testfixture:** `crawl-status-codes`: Route `/code/{n}` liefert Status n für n ∈ {200, 201, 204, 206, 218, 299, 301, 302, 303, 304, 307, 308, 400, 401, 403, 404, 410, 418, 429, 451, 499, 500, 502, 503, 504, 520, 600, 999}; `/early-hints` (103 dann 200).

### CRAWL-013 Klassifikation „Keine Antwort“
- **Priorität:** P0
- **Beschreibung:** Kommt keine HTTP-Antwort zustande, wird die URL mit Statuscode 0 und einer klar unterscheidbaren Ursache gespeichert.
- **Eingaben/Konfiguration:** Antwort-Timeout (CRAWL-095).
- **Ausgabe:** `Statuscode` = 0; `Status` ∈ {„DNS-Auflösung fehlgeschlagen“, „Verbindung abgelehnt“, „Verbindungs-Timeout“, „Antwort-Timeout“, „Verbindungsfehler“, „TLS-Fehler“, „Ungültige URL“, „Durch robots.txt blockiert“, „Zu viele Weiterleitungen“}.
- **Akzeptanzkriterien:**
  - [ ] Nicht auflösbarer Host → „DNS-Auflösung fehlgeschlagen“.
  - [ ] Port geschlossen (RST) → „Verbindung abgelehnt“.
  - [ ] TCP-Aufbau hängt → „Verbindungs-Timeout“ nach Ablauf des Timeouts.
  - [ ] Verbindung steht, aber keine Antwort innerhalb des Antwort-Timeouts → „Antwort-Timeout“.
  - [ ] Verbindung wird mitten im Body abgebrochen → „Verbindungsfehler“ (bereits empfangener Statuscode wird verworfen, OS-Festlegung).
  - [ ] Ungültiges/abgelaufenes Zertifikat → „TLS-Fehler“ (sofern nicht CRAWL-023 anders konfiguriert).
  - [ ] Alle diese Fälle erscheinen im Filter „Keine Antwort“; „Durch robots.txt blockiert“ erscheint ausschließlich im eigenen Filter, nicht zusätzlich unter „Keine Antwort“ (Referenz-Fix 23.1).
- **Testfixture:** `crawl-no-response`: Links auf `nicht-existent.invalid`, `site.test:1` (geschlossen), `/hang-connect` (Server nimmt Verbindung nicht an, via Blackhole-Port), `/hang-response` (antwortet nie), `/reset-mid-body`, `https://expired.site.test/` (abgelaufenes Zertifikat).

### CRAWL-014 HTTP-Weiterleitungen (3xx)
- **Priorität:** P0
- **Beschreibung:** Bei 301, 302, 303, 307, 308 wird das `Location`-Ziel aufgelöst und – sofern im Bereich und nicht ausgeschlossen – als neue URL eingeplant. Die weiterleitende URL wird selbst als Zeile mit Ziel gespeichert.
- **Eingaben/Konfiguration:** Limits › „Max. Weiterleitungen“ (CRAWL-072); Include/Exclude; Crawl-Bereich.
- **Ausgabe:** Spalten `Weiterleitungs-URL` `redirect_url`, `Weiterleitungstyp` `redirect_type` = „HTTP-Weiterleitung“.
- **Akzeptanzkriterien:**
  - [ ] Relatives `Location` wird gegen die anfragende URL aufgelöst.
  - [ ] 3xx ohne `Location` wird als 3xx gespeichert, `Weiterleitungs-URL` leer, kein Folge-Request.
  - [ ] Weiterleitungsziel auf externen Host: Ziel landet im Tab „Extern“ und wird einmal geprüft.
  - [ ] Weiterleitungsziel, das per Exclude ausgeschlossen ist, wird nicht angefragt; `Weiterleitungs-URL` ist trotzdem befüllt.
  - [ ] Weiterleitungsziel wird über die normale Deduplizierung eingeplant (ein bereits gecrawltes Ziel wird nicht erneut angefragt).
  - [ ] Der Body einer 3xx-Antwort wird nicht nach Links geparst (OS-Festlegung, siehe Q).
  - [ ] `Location` mit Leerzeichen oder Nicht-ASCII-Zeichen wird prozentkodiert.
- **Testfixture:** `crawl-redirects`: `/r301`→`/ziel`, `/r302`, `/r303`, `/r307`, `/r308`, `/r-relativ` (Location `../ziel`), `/r-leer` (301 ohne Location), `/r-extern` → `https://other.test/`, `/r-excluded` → `/privat/` (Exclude `.*/privat/.*`), `/r-umlaut` → `/zïel`.

### CRAWL-015 Meta-Refresh- und HTTP-Refresh-Weiterleitungen
- **Priorität:** P0
- **Beschreibung:** Ein `<meta http-equiv="refresh" content="N; url=…">` im HTML sowie ein HTTP-Header `Refresh: N; url=…` werden als Weiterleitungen erkannt.
- **Eingaben/Konfiguration:** Spider › Crawl › „Meta Refresh“ speichern/crawlen (CRAWL-050).
- **Ausgabe:** Weiterleitungstyp „Meta Refresh“ bzw. „HTTP Refresh“; Filter „Weiterleitung (Meta Refresh)“ und „Weiterleitung (HTTP Refresh)“; der Statuscode der Seite bleibt (meist 200).
- **Akzeptanzkriterien:**
  - [ ] Varianten werden erkannt: `0;url=/x`, `0; URL='/x'`, `5;url="/x"`, ohne Leerzeichen, mit Großbuchstaben im `http-equiv`.
  - [ ] Ein Refresh ohne URL (`content="30"`) ist keine Weiterleitung, sondern ein Selbst-Refresh (relevant für CRAWL-094).
  - [ ] Nur das erste Meta-Refresh-Tag im Dokument zählt.
  - [ ] Bei gleichzeitiger HTTP-3xx-Antwort hat die HTTP-Weiterleitung Vorrang.
  - [ ] HTTP-Refresh-Header wird mit derselben Syntax geparst.
  - [ ] Die Verzögerung in Sekunden wird in der Spalte `Meta-Refresh-Verzögerung` (OS-Erweiterung) gespeichert.
- **Testfixture:** `crawl-refresh`: `/meta0` (0;url=/ziel), `/meta5`, `/meta-quotes`, `/meta-self` (nur „30“), `/meta-two` (zwei Tags), `/http-refresh` (Header), `/meta-and-301`.

### CRAWL-016 JavaScript-Weiterleitungen
- **Priorität:** P1
- **Beschreibung:** Nur bei JS-Rendering: Navigiert die Seite während des Renderns per Skript (`location.href`, `location.replace`, `window.location = …`) auf eine andere URL, wird dies als Weiterleitung erfasst.
- **Eingaben/Konfiguration:** Rendering = JavaScript.
- **Ausgabe:** Weiterleitungstyp „JavaScript-Weiterleitung“; Filter „Weiterleitung (JavaScript)“.
- **Akzeptanzkriterien:**
  - [ ] Navigation innerhalb des AJAX-Timeouts wird erkannt; Ziel wird als neue URL eingeplant.
  - [ ] Reine `history.pushState`-Änderungen gelten nicht als Weiterleitung.
  - [ ] Navigation nach Ablauf des Timeouts wird ignoriert.
  - [ ] Im Text-Modus wird keine JS-Weiterleitung gemeldet.
- **Testfixture:** `crawl-js-redirect`: `/js-href` (sofort), `/js-replace` (nach 1 s), `/js-late` (nach 10 s, AJAX-Timeout 5 s), `/js-pushstate`.

### CRAWL-017 Weiterleitungsketten und -schleifen
- **Priorität:** P0
- **Beschreibung:** Folgen mehrere Weiterleitungen aufeinander, wird die Kette erfasst. Eine Schleife liegt vor, wenn ein Ziel auf eine frühere URL der Kette zurückführt.
- **Eingaben/Konfiguration:** Limits › „Max. Weiterleitungen“.
- **Ausgabe:** Filter „Weiterleitungskette“ (URL leitet auf URL, die selbst weiterleitet) und „Weiterleitungsschleife“; die vollständige Kette ist über den Bericht „Weiterleitungen › Ketten“ verfügbar (Spalten: `Quelle`, `Anzahl Weiterleitungen`, `Schleife` ja/nein, je Hop `Adresse n`, `Statuscode n`, `Weiterleitungstyp n`, `Finale Adresse`, `Finaler Statuscode`).
- **Akzeptanzkriterien:**
  - [ ] Kette `/a`→`/b`→`/c`→`/d`(200): `/a` und `/b` im Filter „Weiterleitungskette“, `/c` nicht (leitet auf 200 weiter).
  - [ ] Schleife `/x`→`/y`→`/x`: beide im Filter „Weiterleitungsschleife“; kein endloses Anfragen (je URL genau ein Request).
  - [ ] Selbstweiterleitung `/s`→`/s` ist eine Schleife.
  - [ ] Ketten über verschiedene Typen (301 → Meta Refresh → 302) werden zusammenhängend erfasst.
  - [ ] Ketten über Hosts hinweg (intern → extern → intern) werden erfasst; externe Zwischenschritte werden abgerufen, deren Ziel wieder intern ist, wird normal gecrawlt.
- **Testfixture:** `crawl-redirect-chains`: `/a`…`/d`, `/x`↔`/y`, `/s`→`/s`, `/mix1` (301) → `/mix2` (Meta Refresh) → `/mix3` (302) → `/ok`, `/ext-hop` → `other.test/back` → `site.test/ok`.

### CRAWL-018 HSTS-Richtlinie respektieren
- **Priorität:** P1
- **Beschreibung:** Sendet ein HTTPS-Host einen `Strict-Transport-Security`-Header, werden spätere `http://`-Links auf diesen Host intern auf HTTPS umgeschrieben, ohne den HTTP-Request auszuführen – wie ein Browser.
- **Eingaben/Konfiguration:** Erweitert › „HSTS-Richtlinie respektieren“ (Referenz-Default (abgeleitet): an – die Doku beschreibt nur, dass die Option abgeschaltet werden kann, nennt den Standard aber nicht ausdrücklich; siehe Q26).
- **Ausgabe:** Die `http://`-URL erhält Statuscode 307, Status „HSTS-Richtlinie“, Weiterleitungstyp „HSTS-Richtlinie“.
- **Akzeptanzkriterien:**
  - [ ] Option an: Nach dem ersten HTTPS-Abruf mit HSTS-Header wird `http://site.test/x` nicht angefragt (Request-Log leer für Port 80), aber als 307 → `https://site.test/x` gespeichert.
  - [ ] `includeSubDomains` wird für Subdomains berücksichtigt; `max-age=0` hebt die Richtlinie auf.
  - [ ] HSTS-Header über HTTP (nicht HTTPS) wird ignoriert.
  - [ ] Option aus: Der echte HTTP-Request wird ausgeführt und dessen wirklicher Statuscode (z. B. 301) gespeichert.
  - [ ] HSTS-Zustand gilt nur für den laufenden Crawl (keine Persistenz zwischen Crawls, keine Preload-Liste).
- **Testfixture:** `crawl-hsts`: `https://site.test/` mit `Strict-Transport-Security: max-age=31536000; includeSubDomains`, verlinkt auf `http://site.test/x` und `http://blog.site.test/`; `http://site.test/x` liefert 301.

### CRAWL-019 Inhaltstyp-Erkennung
- **Priorität:** P0
- **Beschreibung:** Der Inhaltstyp jeder URL wird aus dem `Content-Type`-Header bestimmt und einer Kategorie zugeordnet, die Parsing und Filterzuordnung steuert.
- **Eingaben/Konfiguration:** CRAWL-020.
- **Ausgabe:** Spalte `Inhaltstyp` `content_type` (Header-Wert ohne Parameter-Änderung); Kategorien für die Filter der Tabs „Intern“/„Extern“: HTML, JavaScript, CSS, Bilder, PDF, Flash, Medien, Schriftarten, XML, Sonstige, Unbekannt.
- **Akzeptanzkriterien:**
  - [ ] HTML: `text/html`, `application/xhtml+xml`.
  - [ ] JavaScript: `application/javascript`, `text/javascript`, `application/x-javascript`, `application/ecmascript`, `text/ecmascript`, `module`.
  - [ ] CSS: `text/css`. Bilder: `image/*`. PDF: `application/pdf`. Flash: `application/x-shockwave-flash`. Medien: `audio/*`, `video/*`. Schriftarten: `font/*`, `application/font-*`, `application/vnd.ms-fontobject`. XML: `application/xml`, `text/xml`, `application/rss+xml`, `application/atom+xml`.
  - [ ] Header-Vergleich ist case-insensitiv; Parameter (`; charset=…`) werden für die Kategorie ignoriert.
  - [ ] Fehlender oder leerer `Content-Type` → Kategorie „Unbekannt“; ebenso URLs ohne Antwort und durch robots.txt blockierte URLs.
  - [ ] Kein Content-Sniffing am Body, außer bei aktivem CRAWL-020 (OS-Festlegung: auch kein Sniffing bei offensichtlich falschem Header).
  - [ ] Nur Kategorie HTML wird nach Links geparst (plus PDF gemäß CRAWL-057, XML-Sitemaps gemäß CRAWL-060, CSS gemäß CRAWL-040).
- **Testfixture:** `crawl-content-types`: je eine Route pro oben genanntem MIME-Typ, `/kein-ct` (ohne Header, HTML-Body), `/falscher-ct` (`text/plain` mit HTML-Body), `/HTML-upper` (`Text/HTML; Charset=UTF-8`).

### CRAWL-020 „Seiten als HTML annehmen“
- **Priorität:** P1
- **Beschreibung:** Liefert ein Server keinen `Content-Type`, kann die Engine solche Antworten als HTML behandeln und parsen.
- **Eingaben/Konfiguration:** Erweitert › „Seiten als HTML annehmen“ (Standard: aus).
- **Ausgabe:** Kategorie HTML statt „Unbekannt“; `Inhaltstyp` bleibt leer.
- **Akzeptanzkriterien:**
  - [ ] Option aus: `/kein-ct` erscheint unter „Unbekannt“, seine Links werden nicht gecrawlt.
  - [ ] Option an: `/kein-ct` erscheint unter „HTML“, Titel wird extrahiert, Links werden gecrawlt.
  - [ ] Option wirkt nicht auf Antworten mit gesetztem, aber falschem Content-Type (`/falscher-ct` bleibt „Sonstige“).
- **Testfixture:** `crawl-content-types` (`/kein-ct` mit Link auf `/nur-von-kein-ct`).

### CRAWL-021 Zeichensatz-Erkennung
- **Priorität:** P0
- **Beschreibung:** Der Zeichensatz eines HTML-Dokuments wird in fester Reihenfolge bestimmt: BOM, `charset` im `Content-Type`-Header, `<meta charset>` bzw. `<meta http-equiv="Content-Type">` in den ersten 1024 Bytes, sonst UTF-8 mit Rückfall auf Windows-1252 bei ungültigen Sequenzen (OS-Festlegung nach WHATWG-Encoding-Standard).
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** Spalte `Zeichensatz` `charset` (OS-Erweiterung); korrekt dekodierte Titel, Überschriften, Linktexte.
- **Akzeptanzkriterien:**
  - [ ] BOM hat Vorrang vor Header und Meta.
  - [ ] Header-Charset hat Vorrang vor Meta-Charset.
  - [ ] ISO-8859-1-, Windows-1252-, Shift_JIS-, EUC-KR-, GB18030- und UTF-16-Seiten werden korrekt dekodiert (Titel enthält erwartete Zeichen).
  - [ ] Unbekannter Charset-Name (`charset=foo`) fällt auf die nächste Stufe zurück.
  - [ ] Aliase gemäß WHATWG werden aufgelöst (`latin1` → Windows-1252).
  - [ ] Ungültige Bytefolgen werden durch U+FFFD ersetzt, nie Absturz.
- **Testfixture:** `crawl-charsets`: `/utf8-bom`, `/latin1-header`, `/meta-sjis`, `/header-vs-meta` (Header UTF-8, Meta latin1, Body UTF-8), `/utf16le-bom`, `/unknown-charset`, `/invalid-bytes`; Titel enthält jeweils „Grüße ÄÖÜ ß“ bzw. passende CJK-Zeichen.

### CRAWL-022 Antwortzeit, Größe, Last-Modified
- **Priorität:** P0
- **Beschreibung:** Für jede URL werden Zeitmessung und Größenangaben erfasst.
- **Eingaben/Konfiguration:** Extraktion › URL-Details (CRAWL-062): Antwortzeit und Last-Modified sind abschaltbar; Größen und Zeitstempel werden immer erfasst.
- **Ausgabe:**
  - `Antwortzeit (s)` `response_time`: Zeit vom Absenden des ersten Request-Bytes (nach Verbindungsaufbau aus dem Pool bzw. inkl. DNS/TCP/TLS, wenn eine neue Verbindung nötig ist) bis zum Empfang des letzten Body-Bytes; ohne Warteschlangen-, Drossel- und Renderzeit.
  - `Größe (Bytes)` `size`: Länge des dekomprimierten Bodys (ohne Header).
  - `Übertragen (Bytes)` `transferred`: tatsächlich über die Leitung empfangene Bytes für diese eine URL (Header + komprimierter Body).
  - `Gesamt übertragen (Bytes)` `total_transferred` (Referenz-Spalte des Tabs „Intern“): Summe aus `Übertragen` der Seite und `Übertragen` aller Ressourcen, die zum Darstellen der Seite geladen werden. Im JS-Modus = alle vom Browser für diese Seite geladenen Ressourcen (Netzwerk-Mitschnitt, Cache-Treffer zählen mit ihrer Originalgröße); im Textmodus = Seite + alle eingebundenen, gecrawlten Bilder/CSS/JS/Medien/Schriftarten (je Ressource einmal, auch wenn mehrfach eingebunden). Grundlage für die CO2-Schätzung (CRAWL-139).
  - `Last-Modified` `last_modified`: Header-Wert als Datum (UTC), `Gecrawlt am` `crawl_timestamp` (UTC, ISO 8601, Zeitpunkt des Request-Beginns).
- **Akzeptanzkriterien:**
  - [ ] Antwort mit 800 ms künstlicher Verzögerung vor dem ersten Byte zeigt `Antwortzeit` ≥ 0,800 s und ≤ 1,000 s (unbelastete Testmaschine, 1 Thread).
  - [ ] Bei aktiver URL-Drosselung (CRAWL-113, 0,5 URLs/s) steigt die gemessene `Antwortzeit` der `/fast`-Route nicht über 0,100 s (Wartezeit wird nicht mitgemessen).
  - [ ] Zeit wird auf Millisekunden genau gespeichert und mit drei Nachkommastellen angezeigt (`0,812`).
  - [ ] `/gzip-50k` (50 000 Bytes HTML, gzip auf ca. 5 000 Bytes): `Größe` = 50 000; `Übertragen` = Header-Bytes + komprimierte Body-Bytes (Abweichung zum Byte-Zähler des Fixture-Servers ≤ 0).
  - [ ] `/with-assets` (HTML 10 000 B unkomprimiert, bindet `a.png` 20 000 B und `b.css` 5 000 B ein, keine Kompression): `Gesamt übertragen` = Summe der drei `Übertragen`-Werte; bindet die Seite `a.png` zweimal ein, ändert sich der Wert nicht.
  - [ ] Ist das Crawlen von Bildern abgeschaltet, zählen Bilder im Textmodus nicht zu `Gesamt übertragen` (Wert = Seite + gecrawlte Ressourcen); die Spalte trägt dann den Tooltip „unvollständig – Ressourcen nicht gecrawlt“.
  - [ ] Für Nicht-HTML-URLs (Bild, CSS) gilt `Gesamt übertragen` = `Übertragen`.
  - [ ] Ungültiges `Last-Modified` (`gestern`) → Spalte leer, kein Fehler; gültiges RFC-1123-Datum wird als `2015-10-21 07:28:00` (UTC) gespeichert.
  - [ ] Bei Weiterleitungen gilt die Zeit des jeweiligen einzelnen Requests, nicht der ganzen Kette (`/r301` → `/slow-800`: Zeile `/r301` < 0,100 s).
  - [ ] Antwortzeit-Erfassung aus (CRAWL-062): Spalte `Antwortzeit` leer für alle URLs, kein Wert 0.
- **Testfixture:** `crawl-timing`: `/slow-800` (delay 800 ms), `/fast`, `/gzip-50k`, `/with-assets` (+ `a.png`, `b.css`), `/r301` → `/slow-800`, `/lm-valid` (`Last-Modified: Wed, 21 Oct 2015 07:28:00 GMT`), `/lm-invalid` (`Last-Modified: gestern`).

### CRAWL-023 TLS und Zertifikate
- **Priorität:** P1
- **Beschreibung:** HTTPS-Verbindungen werden standardmäßig gegen den Betriebssystem-Zertifikatsspeicher geprüft. Für Firmennetze mit TLS-Inspektion (MITM-Proxy) können zusätzliche vertrauenswürdige Zertifikate hinterlegt werden.
- **Eingaben/Konfiguration:** Einstellungen › „Vertrauenswürdige Zertifikate“ (Liste von PEM/DER-Dateien; Schaltfläche „Zertifikate ermitteln“, die die Kette einer frei wählbaren Test-URL ausliest und zum Übernehmen anbietet); Erweitert › „Zertifikatsfehler ignorieren“ (OS-Erweiterung, Standard: aus).
- **Ausgabe:** Bei Fehlern Status „TLS-Fehler“ mit Detail (abgelaufen, falscher Hostname, unbekannte CA, selbstsigniert).
- **Akzeptanzkriterien:**
  - [ ] Abgelaufenes, selbstsigniertes und Hostname-falsches Zertifikat erzeugen jeweils „TLS-Fehler“ mit unterscheidbarem Detail.
  - [ ] Nach Import des selbstsignierten CA-Zertifikats wird die Seite mit 200 gecrawlt.
  - [ ] „Zertifikatsfehler ignorieren“ an: alle drei Fälle werden mit dem echten HTTP-Status gecrawlt; im UI ist ein Warnhinweis sichtbar.
  - [ ] TLS 1.2 und 1.3 werden unterstützt; SNI wird gesendet.
- **Testfixture:** `crawl-tls`: `expired.site.test`, `selfsigned.site.test`, `wronghost.site.test` (Zertifikat für `other.test`), CA-Datei `tests/fixtures/crawl-tls/ca.pem`.

### CRAWL-024 HTTP-Versionen (HTTP/1.1, HTTP/2)
- **Priorität:** P1 (Spalte `HTTP-Version`) / P2 (Auswahlschalter)
- **Beschreibung:** Die Referenz führt im Tab „Intern“ bereits eine Spalte `HTTP-Version` mit der Protokollversion der Antwort; diese Spalte ist damit **Paritätsumfang** (Priorität der Spalte: P1). Eine **wählbare** HTTP-Version dokumentiert die Referenz dagegen nicht – der Auswahlschalter ist eine OS-Erweiterung (P2). OpenSpider unterstützt HTTP/1.1 und HTTP/2 (per ALPN).
- **Eingaben/Konfiguration:** Erweitert › „HTTP-Version“ (OS-Erweiterung): Automatisch (Standard, ALPN) / Nur HTTP/1.1 / HTTP/2 bevorzugen.
- **Ausgabe:** Spalte `HTTP-Version` `http_version` (Referenz-Spalte; Werte `HTTP/1.0`, `HTTP/1.1`, `HTTP/2`; leer bei Statuscode 0).
- **Akzeptanzkriterien:**
  - [ ] Server mit h2-ALPN wird im Modus „Automatisch“ mit HTTP/2 angesprochen; Spalte zeigt `HTTP/2`.
  - [ ] „Nur HTTP/1.1“ erzwingt HTTP/1.1 auch gegen h2-fähige Server.
  - [ ] Statuscodes und Header-Auswertung sind zwischen HTTP/1.1 und HTTP/2 identisch (gleiche Fixture, gleiche Ergebnisse).
  - [ ] Unverschlüsseltes HTTP/2 (h2c) wird nicht verwendet.
- **Testfixture:** `crawl-http2`: `https://h2.site.test/` (h2 + http/1.1), `https://h1only.site.test/`.

### CRAWL-025 IPv6
- **Priorität:** P2
- **Beschreibung:** Hosts mit AAAA-Einträgen und IPv6-Literal-URLs werden unterstützt. Bei Dual-Stack-Hosts wird „Happy Eyeballs“ verwendet.
- **Eingaben/Konfiguration:** Erweitert › „IP-Version“: Automatisch (Standard) / Nur IPv4 / Nur IPv6.
- **Ausgabe:** Spalte `IP-Adresse` `ip_address` (OS-Erweiterung, tatsächlich verbundene Adresse).
- **Akzeptanzkriterien:**
  - [ ] `http://[::1]:PORT/` wird gecrawlt; Host-Darstellung in eckigen Klammern bleibt erhalten.
  - [ ] Host nur mit AAAA-Eintrag wird im Modus „Automatisch“ erreicht.
  - [ ] „Nur IPv4“ bei einem reinen IPv6-Host führt zu „DNS-Auflösung fehlgeschlagen“.
- **Testfixture:** `crawl-ipv6`: Fixture-Server zusätzlich auf `::1`; Host-Überschreibung `v6only.site.test` → `::1`.

---
## B. Crawl-Modi

### CRAWL-026 Spider-Modus
- **Priorität:** P0
- **Beschreibung:** Standardmodus. Ausgehend von einer Start-URL werden Links rekursiv im gewählten Bereich verfolgt, bis die Warteschlange leer ist oder ein Limit greift.
- **Eingaben/Konfiguration:** Menü „Modus“ › Spider (Standard); Start-URL; gesamte Konfiguration.
- **Ausgabe:** alle Tabs; Statusleiste mit Fortschritt (CRAWL-130).
- **Akzeptanzkriterien:**
  - [ ] Nach Moduswechsel bleibt die Konfiguration erhalten; die Ergebnistabellen werden nach Rückfrage geleert; der bisherige Crawl bleibt als Datei in der Crawl-Verwaltung erhalten (CRAWL-125).
  - [ ] Der Crawl endet automatisch, wenn keine URL mehr in der Warteschlange ist und alle Threads idle sind; Statusleiste zeigt „Abgeschlossen“.
  - [ ] Eine Fixture mit 100 zusammenhängenden Seiten ergibt exakt 100 Zeilen im Filter „Intern › HTML“.
- **Testfixture:** `crawl-site-100`: generierte Website mit 100 Seiten, Baumstruktur Tiefe 4, plus 10 Bilder, 3 CSS, 3 JS.

### CRAWL-027 Listen-Modus: Eingabequellen
- **Priorität:** P0
- **Beschreibung:** Statt einer Start-URL wird eine Liste von URLs geprüft. Die Liste kann eingefügt, aus Dateien geladen oder aus einer XML-Sitemap gelesen werden.
- **Eingaben/Konfiguration:** Modus › Liste; Schaltfläche „Hochladen“ mit den Quellen: **Aus Datei**, **Manuell eingeben** (mehrzeiliges Textfeld zum Tippen/Bearbeiten vor der Übernahme), **Einfügen** (übernimmt den Zwischenablage-Inhalt direkt ohne Zwischendialog), **XML-Sitemap herunterladen** (eine URL), **Mehrere XML-Sitemaps herunterladen** (mehrere URLs ohne Index-Datei), **Google-Tabelle** (P2; Konto + Tabellen-URL, Integration im API-Bereich).
- **Ausgabe:** Zusammenfassungsdialog mit Zählern: `Gelesen` (alle als URL erkannten Zeichenketten), `Ungültig` (erkannte, aber nicht parsbare URLs), `Ignorierte Zeilen` (ohne URL), `Duplikate entfernt`, `Durch Exclude entfernt`, `Durch Include entfernt`, `Zu crawlen` (= Gelesen − Ungültig − Duplikate − Exclude − Include). Schaltflächen „OK“ (Crawl starten) und „Abbrechen“.
- **Akzeptanzkriterien:**
  - [ ] Dateiformate `.txt`, `.csv`, `.tsv`, `.xls`, `.xlsx`, `.xml` (Sitemap oder Sitemap-Index), `.xml.gz` werden gelesen; andere Endungen werden als Text versucht.
  - [ ] Aus beliebigem Text werden alle Zeichenketten extrahiert, die mit `http://` oder `https://` beginnen (Schema case-insensitiv, `HTTPS://` wird erkannt) und bis zum nächsten Whitespace, `"`, `<`, `>` oder Zeilenende reichen; mehrere URLs in einer Zeile ergeben mehrere Einträge.
  - [ ] Eingaben ohne Schema (`www.site.test/a`) werden nicht erkannt und als `Ignorierte Zeilen` gezählt (Abweichung zur Start-URL in CRAWL-001, OS-Festlegung, siehe Q27).
  - [ ] In CSV/Excel werden alle Zellen aller Spalten und aller Tabellenblätter durchsucht; Kopfzeilen ohne URL werden ignoriert.
  - [ ] Sitemap-Index wird rekursiv aufgelöst; gzip-komprimierte Sitemaps werden gelesen; die Sitemap-Dateien selbst erscheinen nicht als zu crawlende Listen-URLs.
  - [ ] „Einfügen“ und „Manuell eingeben“ liefern bei gleichem Text dieselbe Zählung.
  - [ ] Fixture `dupes.txt` (10 URLs, 3 Duplikate) mit Exclude `.*/privat/.*` (1 Treffer): Dialog zeigt `Gelesen` 10, `Duplikate entfernt` 3, `Durch Exclude entfernt` 1, `Zu crawlen` 6.
  - [ ] Die Duplikat-Erkennung nutzt die normalisierte Form (CRAWL-003): `HTTPS://SITE.test/a` und `https://site.test/a` sind Duplikate.
  - [ ] Eine Liste mit 100 000 URLs wird in unter 10 s eingelesen (Referenzmaschine: 8 Kerne, SSD).
  - [ ] Nach einem Listen-Crawl aus einer XML-Sitemap sind die sitemapbezogenen Filter (z. B. „In Sitemap“, „Nicht in Sitemap“, „Verwaiste URLs“) erst nach Abschluss der Crawl-Analyse (CRAWL-141) befüllt; vorher zeigen sie den Hinweis „Crawl-Analyse erforderlich“ statt 0.
- **Testfixture:** `crawl-list-input`: Dateien `urls.txt` (gemischt mit Kommentarzeilen, zwei URLs in einer Zeile, `HTTPS://`, `www.site.test/ohne-schema`), `urls.csv` (URLs in Spalte 3), `urls.xlsx` (zwei Blätter), `sitemap-index.xml` → zwei Kind-Sitemaps (eine davon `.gz`), `dupes.txt` (10 URLs, 3 doppelt, 1 unter `/privat/`).

### CRAWL-028 Listen-Modus: Verhalten
- **Priorität:** P0
- **Beschreibung:** Im Listenmodus werden genau die gelieferten URLs geprüft. Andere Standardwerte gelten als im Spider-Modus.
- **Eingaben/Konfiguration:** Limits › Crawl-Tiefe (im Listenmodus Standard 0); robots.txt-Modus (im Listenmodus Standard „Ignorieren“).
- **Ausgabe:** Tabs wie im Spider-Modus; `Crawl-Tiefe` = 0 für alle Listen-URLs.
- **Akzeptanzkriterien:**
  - [ ] Standard: nur die Listen-URLs werden angefragt; Links aus den Seiten werden extrahiert und in den Outlinks gespeichert, aber nicht angefragt.
  - [ ] Ressourcen der Seiten (Bilder/CSS/JS) werden ebenfalls nicht angefragt (Tiefe-Limit 0 gilt; OS-Festlegung, siehe Q).
  - [ ] Wird die Tiefe auf 1 gesetzt, werden zusätzlich alle direkt verlinkten URLs geprüft.
  - [ ] robots.txt wird standardmäßig ignoriert; wird im Listenmodus „Respektieren“ gewählt, werden blockierte Listen-URLs als „Durch robots.txt blockiert“ gemeldet.
  - [ ] Listen-URLs auf verschiedenen Hosts werden alle als intern behandelt (jeder Host der Liste ist im Bereich).
  - [ ] Doppelte URLs werden einmal gecrawlt.
  - [ ] Weiterleitungen werden nicht verfolgt, außer CRAWL-030 ist aktiv oder Tiefe ≥ 1.
- **Testfixture:** `crawl-list-behavior`: Liste aus `site.test/a`, `site.test/robots-blocked`, `other.test/b`, `site.test/r301`; `/a` verlinkt `/a-kind` und bindet `bild.png` ein.

### CRAWL-029 Listen-Modus: Export in Upload-Reihenfolge
- **Priorität:** P1
- **Beschreibung:** Ein eigener Export im Listenmodus gibt die Ergebnisse exakt in der Reihenfolge und Anzahl der hochgeladenen Liste aus – inklusive Duplikaten und automatisch korrigierter Eingaben.
- **Eingaben/Konfiguration:** Schaltfläche „Exportieren“ neben „Hochladen“ im Listenmodus.
- **Ausgabe:** CSV/XLSX mit Spalte `Originaleingabe` und allen Spalten des Tabs „Intern“; eine Zeile pro Eingabezeile.
- **Akzeptanzkriterien:**
  - [ ] Eingabe mit 10 Zeilen (3 Duplikate) → Export mit 10 Datenzeilen in Originalreihenfolge.
  - [ ] Eingabe, die normalisiert wurde (`HTTPS://SITE.test/a`), zeigt in `Originaleingabe` den Rohwert und in `Adresse` die normalisierte URL.
  - [ ] Durch Exclude entfernte Eingaben erscheinen mit leerem Statuscode und Status „Ausgeschlossen“ (OS-Festlegung).
- **Testfixture:** `crawl-list-input/dupes.txt`.

### CRAWL-030 Weiterleitungen/Canonicals immer bis zum Ziel folgen
- **Priorität:** P1
- **Beschreibung:** Für Migrationsprüfungen: Weiterleitungen bzw. Canonical-Ketten werden unabhängig vom Tiefenlimit bis zum Endziel verfolgt; jeder Zwischenschritt wird erfasst.
- **Eingaben/Konfiguration:** Erweitert › „Weiterleitungen immer folgen“ (Standard: aus), „Canonicals immer folgen“ (Standard: aus).
- **Ausgabe:** Bericht „Weiterleitungen › Ketten“ bzw. „Canonicals › Ketten“; Spalten `Finale Adresse`, `Finaler Statuscode`, `Anzahl Weiterleitungen`.
- **Akzeptanzkriterien:**
  - [ ] Listenmodus Tiefe 0, Option an: `/a`→`/b`→`/c`(200) – alle drei werden angefragt; ohne Option nur `/a`.
  - [ ] Das Limit „Max. Weiterleitungen“ (CRAWL-072) gilt weiterhin.
  - [ ] Canonical-Option: `/p1` (canonical `/p2`), `/p2` (canonical `/p3`), `/p3` (selbstreferenzierend) – alle drei werden angefragt.
  - [ ] Externe Ziele werden ebenfalls bis zum Ende verfolgt.
  - [ ] Im Spider-Modus wirkt die Option ebenso (Ziele jenseits des Tiefenlimits werden trotzdem verfolgt).
- **Testfixture:** `crawl-migration`: Liste mit `/a`, `/p1`; Ketten wie beschrieben; `/ext` → `other.test/x` → `other.test/y`.

### CRAWL-031 SERP-Modus
- **Priorität:** P1
- **Beschreibung:** Ohne zu crawlen werden hochgeladene Titel und Meta-Beschreibungen auf Zeichenlänge und Pixelbreite geprüft – etwa für Entwürfe vor dem Livegang. Die Längen-/Pixellogik selbst ist im Bereich Seitentitel/Meta-Beschreibung spezifiziert; hier geht es um den Modus.
- **Eingaben/Konfiguration:** Modus › SERP; Upload von `.txt`, `.csv`, `.xls`, `.xlsx` mit Kopfzeilen `URL`, `Title`, `Description` (zusätzlich deutsche Aliase `Titel`, `Beschreibung`; case-insensitiv).
- **Ausgabe:** Tabs „Seitentitel“ und „Meta-Beschreibung“ mit Längen-, Pixel- und Duplikat-Filtern; Bericht „SERP-Zusammenfassung“ im wieder hochladbaren Format (Spalten `URL`, `Title`, `Description` plus Längen- und Pixelspalten).
- **Akzeptanzkriterien:**
  - [ ] Es wird keine einzige HTTP-Anfrage gesendet (Request-Log bleibt leer).
  - [ ] Zeilen ohne URL werden mit generischem Platzhalter (`Zeile 7`) geführt (OS-Festlegung).
  - [ ] Ein Export der SERP-Zusammenfassung kann unverändert erneut hochgeladen werden und ergibt identische Werte (Round-Trip).
  - [ ] Fehlende Pflichtkopfzeile führt zu einer verständlichen Fehlermeldung, welche Spalte fehlt.
  - [ ] Unicode-Inhalte (Umlaute, Emoji, CJK) werden korrekt gemessen.
- **Testfixture:** `crawl-serp`: `serp.csv` mit 5 Zeilen (zu kurzer, zu langer, passender Titel; doppelte Beschreibung; Emoji-Titel), `serp-missing-header.csv`.

### CRAWL-032 Vergleichsmodus: Auswahl und Kategorien
- **Priorität:** P1
- **Beschreibung:** Zwei gespeicherte Crawls werden verglichen. Für jeden Tab und Filter wird gezählt, wie sich die URL-Mengen verändert haben. Da jeder OpenSpider-Crawl gespeichert ist (Konventionen, Speicherarchitektur), ist der Vergleich unabhängig vom Leistungsprofil (CRAWL-124) immer verfügbar.
- **Eingaben/Konfiguration:** Modus › Vergleich; Auswahl „Aktueller Crawl“ und „Vorheriger Crawl“ aus der Crawl-Verwaltung (oder in der Verwaltung zwei Crawls markieren › „Zum Vergleich auswählen“); Tauschen-Schaltfläche.
- **Ausgabe:** Übersicht je Filter mit Spalten `Aktuell`, `Vorher`, `Änderung`, `Hinzugefügt`, `Neu`, `Entfernt`, `Fehlend`; Hauptfenster-Umschalter Aktuell/Vorher und Kategorie-Filter. Detailansichten im unteren Fenster siehe CRAWL-144.
- **Kategorien (formal):** Sei C die URL-Menge des aktuellen, P die des vorherigen Crawls (nach URL-Zuordnung, CRAWL-034), F_C(u) bzw. F_P(u) die Aussage „u erfüllt den betrachteten Filter im jeweiligen Crawl“. Dann gilt:
  - Hinzugefügt = { u ∈ C ∩ P | F_C(u) ∧ ¬F_P(u) }
  - Entfernt = { u ∈ C ∩ P | F_P(u) ∧ ¬F_C(u) }
  - Neu = { u ∈ C \ P | F_C(u) }
  - Fehlend = { u ∈ P \ C | F_P(u) }
  - `Aktuell` = |{ u ∈ C | F_C(u) }|, `Vorher` = |{ u ∈ P | F_P(u) }|, `Änderung` = `Aktuell` − `Vorher`.
- **Akzeptanzkriterien:**
  - [ ] Standardmäßig ist der Crawl mit dem jüngeren Startzeitpunkt „Aktuell“, der ältere „Vorher“; Tauschen vertauscht die Rollen und damit Hinzugefügt ↔ Entfernt und Neu ↔ Fehlend.
  - [ ] Für jeden Filter jedes Tabs gilt die Identität `Aktuell` − `Vorher` = |Hinzugefügt| + |Neu| − |Entfernt| − |Fehlend| (automatisierter Test über alle Filter der Fixture).
  - [ ] Fixture v1→v2, Filter „Seitentitel › Fehlt“: `/c` ∈ Hinzugefügt, `/d` ∈ Entfernt; Filter „Intern › HTML“: `/a` ∈ Neu, `/b` ∈ Fehlend.
  - [ ] Ein Klick auf eine Zahl listet genau diese URLs (Zeilenzahl = angezeigte Zahl).
  - [ ] Der Vergleich wird als eigener Eintrag (Typ „Vergleich“) in der Crawl-Verwaltung gespeichert und kann ohne Neuberechnung geöffnet werden (Öffnen eines gespeicherten Vergleichs mit 2 × 10 000 URLs < 3 s).
  - [ ] Wird einer der beiden Quell-Crawls gelöscht, wird der Vergleich als nicht mehr verfügbar markiert (nicht stillschweigend gelöscht).
  - [ ] Der Vergleichsmodus ist bei jedem Leistungsprofil (CRAWL-124) wählbar; es gibt keinen Zustand, in dem er deaktiviert ist.
- **Testfixture:** `crawl-compare`: zwei Varianten derselben Site (`v1/`, `v2/`), umschaltbar per Fixture-Parameter: in v2 ist `/a` neu, `/b` fehlt, `/c` bekommt einen leeren Titel (Hinzugefügt in „Titel fehlt“), `/d` verliert seinen leeren Titel (Entfernt).

### CRAWL-033 Vergleichsmodus: Änderungserkennung
- **Priorität:** P1
- **Beschreibung:** Zusätzlich zu Filterwechseln kann für ausgewählte Elemente jede inhaltliche Änderung erkannt werden, auch wenn kein Filter anschlägt.
- **Eingaben/Konfiguration:** Konfiguration › Vergleich › Änderungserkennung, auswählbar: Seitentitel, Meta-Beschreibung, H1, Wortanzahl, Crawl-Tiefe, Interne Links (eindeutige Inlinks), Strukturierte Daten (eindeutige Typen/Fehler), Inhalt (Ähnlichkeit via MinHash; erfordert gespeichertes HTML in beiden Crawls).
- **Ausgabe:** Tab „Änderungserkennung“ mit Filtern „Alle“ (URLs in beiden Crawls) und je Element; Umschalter „Geändert“/„Nicht geändert“; Spalten je Element `Aktuell`, `Vorher`, bei numerischen Werten `Änderung` und `Änderung %`; bei Inhalt `Inhaltsänderung %`.
- **Akzeptanzkriterien:**
  - [ ] Nur URLs, die in beiden Crawls vorkommen (nach URL-Zuordnung), werden betrachtet.
  - [ ] Titel „A“ → „B“ erscheint unter „Seitentitel › Geändert“ mit beiden Werten nebeneinander.
  - [ ] Wortanzahl 300 → 450 ergibt `Änderung` = 150 und `Änderung %` = 50.
  - [ ] Inhaltsänderung: identischer Body → 0 %; komplett anderer Body → nahe 100 %; Änderung eines Absatzes von 10 → Wert zwischen 5 und 30 %.
  - [ ] Ist „Inhalt“ gewählt, aber in einem Crawl kein HTML gespeichert, zeigt die Oberfläche einen Hinweis und lässt das Element leer.
  - [ ] Nicht gewählte Elemente erzeugen keinen Filter.
- **Testfixture:** `crawl-compare` mit zusätzlichen Änderungen: `/t` (Titel geändert), `/w` (Wortanzahl), `/content` (ein Absatz geändert), `/depth` (Crawl-Tiefe 1 → 2, weil Link von Startseite entfernt).

### CRAWL-034 Vergleichsmodus: URL-Zuordnung
- **Priorität:** P1
- **Beschreibung:** URLs des vorherigen Crawls werden per Regex auf URLs des aktuellen Crawls abgebildet, damit z. B. Staging gegen Live verglichen werden kann.
- **Eingaben/Konfiguration:** Konfiguration › Vergleich › URL-Zuordnung: Liste von Regeln (Regex, Ersetzung), beliebig viele, in Reihenfolge angewandt; Reiter „Test“ mit Eingabe-URL und Ergebnis.
- **Ausgabe:** zugeordnete URL-Paare gelten als „in beiden Crawls“.
- **Akzeptanzkriterien:**
  - [ ] Regel `https://staging\.site\.test` → `https://www.site.test` ordnet alle Staging-URLs den Live-URLs zu.
  - [ ] Regel für abschließenden Schrägstrich (`^(.*[^/])$` → `$1/`) ordnet `/a` → `/a/` zu.
  - [ ] Mehrere Regeln werden nacheinander angewandt; das Ergebnis der ersten ist Eingabe der zweiten.
  - [ ] Der Test-Reiter zeigt für eine Eingabe die Ausgabe nach jeder Regel.
  - [ ] Ungültiger Regex wird beim Speichern abgewiesen (Fehlermeldung mit Position).
  - [ ] Eine Zuordnung, die mehrere vorherige URLs auf dieselbe aktuelle URL abbildet, verwendet die erste und meldet die Kollision im Log (OS-Festlegung).
- **Testfixture:** `crawl-compare-mapping`: Crawl A auf `staging.site.test` (Pfade ohne Slash), Crawl B auf `www.site.test` (Pfade mit Slash).

### CRAWL-035 Vergleichsmodus: Seitenstruktur-Ansicht
- **Priorität:** P2
- **Beschreibung:** Verzeichnisbaum beider Crawls mit Zählern pro Ordner, wie viele URLs hinzukamen bzw. fehlen, und Verteilung der Crawl-Tiefe.
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** Tab „Seitenstruktur“: Spalten `Pfad`, `Aktuell`, `Vorher`, `Neu` (Pfeil hoch), `Fehlend` (Pfeil runter); aufklappbar bis auf URL-Ebene; Export.
- **Akzeptanzkriterien:**
  - [ ] Ordner `/blog/` mit 3 neuen und 1 fehlenden URL zeigt +3 / −1.
  - [ ] Summen eines Elternordners entsprechen der Summe der Kinder plus eigener URLs.
- **Testfixture:** `crawl-compare` (v1/v2 mit Änderungen in `/blog/`).

### CRAWL-036 Vergleich automatisch (geplante/CLI-Crawls)
- **Priorität:** P2
- **Beschreibung:** Bei geplanten oder per Kommandozeile gestarteten Crawls eines Projekts wird nach Abschluss automatisch mit dem vorherigen Crawl desselben Projekts verglichen.
- **Eingaben/Konfiguration:** Option „Automatisch vergleichen“ in geplanten Aufgaben; CLI `--project-name "<Projekt>" --project-crawl-comparison "true"` (Projekt wird ausschließlich über `--project-name` bestimmt; der Wert von `--project-crawl-comparison` ist ein Wahrheitswert) bzw. `--crawl-comparison <Datenbank-ID 1> <Datenbank-ID 2>` (Rollen Aktuell/Vorher nach Crawl-Datum). Details der CLI-Flags: OUT-202, OUT-152.
- **Ausgabe:** gespeicherter Vergleich; optional Zusammenfassung der Änderungen in der Abschluss-E-Mail.
- **Akzeptanzkriterien:**
  - [ ] Zwei aufeinanderfolgende CLI-Crawls mit `--project-name "P" --project-crawl-comparison "true"` erzeugen genau einen Vergleichseintrag (Crawl 2 aktuell, Crawl 1 vorher).
  - [ ] Beim ersten Crawl eines Projekts wird kein Vergleich erzeugt und kein Fehler gemeldet (Exit-Code 0).
  - [ ] `--project-crawl-comparison "true"` ohne `--project-name` → Exit-Code 1 mit Meldung „Projektname fehlt“.
  - [ ] Andere Werte als `true`/`false` (case-insensitiv) → Exit-Code 1.
- **Testfixture:** `crawl-compare` (zwei CLI-Läufe gegen v1, dann v2).

### CRAWL-037 API-Modus (Daten ohne Crawl)
- **Priorität:** P2
- **Beschreibung:** Für eine hochgeladene URL-Liste werden ausschließlich Daten aus angebundenen Schnittstellen (Analytics, Search Console, PageSpeed, Backlink-APIs) geholt, ohne die URLs selbst abzurufen. Die API-Anbindungen sind in einem eigenen Bereich spezifiziert.
- **Eingaben/Konfiguration:** Modus › APIs; Upload wie Listenmodus.
- **Ausgabe:** API-Spalten in den jeweiligen Tabs.
- **Akzeptanzkriterien:**
  - [ ] Kein Request an die hochgeladenen URLs (Request-Log der Fixture leer).
  - [ ] Ohne verbundene API zeigt der Start einen Hinweis statt eines leeren Laufs.
- **Testfixture:** `crawl-list-input/urls.txt` mit Mock-API-Server.

---

## C. Spider-Konfiguration: Ressourcen- und Seitenlinks

### CRAWL-038 Grundprinzip „Speichern“ vs. „Crawlen“
- **Priorität:** P0
- **Beschreibung:** Für jeden Linktyp gibt es zwei unabhängige Schalter: **Speichern** (die gefundenen URLs bzw. Beziehungen werden in Tabellen und Inlinks/Outlinks geführt) und **Crawlen** (die URLs werden angefragt und – falls HTML und intern – weiter verfolgt). „Crawlen“ ohne „Speichern“ bedeutet: Die URL wird abgerufen und für die Entdeckung genutzt, erscheint aber nicht in den Tabellen.
- **Eingaben/Konfiguration:** Konfiguration › Spider › Crawl, Abschnitte „Ressourcenlinks“ und „Seitenlinks“.
- **Ausgabe:** abhängig vom Linktyp.
- **Akzeptanzkriterien:**
  - [ ] Speichern an, Crawlen aus: URL erscheint in den Tabellen ohne Statuscode (leer), wird nicht angefragt.
  - [ ] Speichern aus, Crawlen an: URL wird angefragt, erscheint nicht in Tabellen; über sie gefundene Seiten erscheinen normal.
  - [ ] Beide aus: URL wird weder angefragt noch angezeigt; der Link ist in den Outlinks der Quellseite nicht vorhanden.
  - [ ] Änderungen während eines pausierten Crawls wirken auf alle noch nicht abgerufenen URLs.
- **Testfixture:** `crawl-store-crawl-matrix`: Seite mit je einem Link jedes Typs (Bild, CSS, JS, Medien, intern, extern, Canonical, next/prev, hreflang, AMP, Meta Refresh, iframe, mobile alternate); Testmatrix über alle 4 Schalterkombinationen je Typ.

### CRAWL-039 Bilder
- **Priorität:** P0
- **Beschreibung:** Bilder, die über `<img src>`, `<picture><source srcset>` (siehe CRAWL-055), `<input type="image">`, `<link rel="icon">` (OS-Festlegung) und Bild-Links `<a href="x.jpg">` eingebunden sind.
- **Eingaben/Konfiguration:** Ressourcenlinks › Bilder: Speichern (Standard: an), Crawlen (Standard: an).
- **Ausgabe:** Tab „Intern/Extern › Bilder“; Bilddaten im Bereich „Bilder“.
- **Akzeptanzkriterien:**
  - [ ] Ein per `<a href>` verlinktes Bild wird als Hyperlink gezählt, nicht als eingebettetes Bild (Spalte `Linktyp` in Inlinks: „Hyperlink“ vs. „Bild“).
  - [ ] CSS-Hintergrundbilder werden nur entdeckt, wenn CSS gecrawlt wird (CRAWL-040) oder JS-Rendering aktiv ist.
  - [ ] Lazy-Load-Attribute (`data-src`) werden im Textmodus nicht als Bild erkannt (OS-Festlegung, siehe Q).
  - [ ] Bilder von externen Hosts werden nur geprüft, wenn „Externe Links › Crawlen“ an ist (CRAWL-045).
- **Testfixture:** `crawl-resources`: `/` mit `<img src>`, `<a href="foto.jpg">`, `<input type=image>`, `<img data-src>`, `style="background:url(bg.png)"`, externes Bild `cdn.other.test/x.png`.

### CRAWL-040 CSS
- **Priorität:** P0
- **Beschreibung:** Stylesheets aus `<link rel="stylesheet">`. Gecrawlte CSS-Dateien werden nach `url(…)` und `@import` durchsucht, die als Links (Bilder, Schriften, weitere CSS) behandelt werden.
- **Eingaben/Konfiguration:** Ressourcenlinks › CSS: Speichern/Crawlen (Standard: an/an).
- **Ausgabe:** Filter „CSS“.
- **Akzeptanzkriterien:**
  - [ ] `@import url("b.css")` und `@import "c.css"` werden gefunden und relativ zur CSS-Datei aufgelöst.
  - [ ] `url(bild.png)`, `url('bild.png')`, `url("bild.png")` werden gefunden; `url(data:…)` nicht.
  - [ ] `<style>`-Blöcke und `style`-Attribute im HTML werden ebenfalls nach `url()` durchsucht.
  - [ ] Schriftarten aus `@font-face` landen unter Kategorie „Schriftarten“.
- **Testfixture:** `crawl-resources/style.css` mit `@import`, Hintergrundbild, `@font-face`, Data-URI.

### CRAWL-041 JavaScript-Dateien
- **Priorität:** P0
- **Beschreibung:** Skripte aus `<script src>` sowie `<link rel="modulepreload">`. Im Textmodus werden JS-Dateien nicht ausgeführt und nicht nach URLs durchsucht.
- **Eingaben/Konfiguration:** Ressourcenlinks › JavaScript: Speichern/Crawlen (Standard: an/an).
- **Ausgabe:** Filter „JavaScript“.
- **Akzeptanzkriterien:**
  - [ ] `<script src>`, `<script type="module" src>`, `<link rel="modulepreload" href>` werden gefunden.
  - [ ] Bei JS-Rendering werden zur Laufzeit nachgeladene Skripte ebenfalls erfasst (Netzwerk-Mitschnitt).
  - [ ] Ist JS-Crawlen aus und Rendering aktiv, zeigt die Oberfläche eine Warnung, weil das Rendering die Dateien trotzdem benötigt (der Renderer lädt sie, sie erscheinen aber nicht in den Tabellen).
- **Testfixture:** `crawl-resources`: `app.js`, `module.mjs`, `lazy.js` (per `import()` nachgeladen).

### CRAWL-042 Medien (Audio/Video)
- **Priorität:** P1
- **Beschreibung:** URLs aus `<video src>`, `<audio src>`, `<source src>` in Video/Audio, `<track src>`, `<video poster>` (als Bild).
- **Eingaben/Konfiguration:** Ressourcenlinks › Medien: Speichern/Crawlen (Standard: an/an; OS-Default, siehe Q).
- **Ausgabe:** Filter „Medien“.
- **Akzeptanzkriterien:**
  - [ ] Alle genannten Attribute werden gefunden; `poster` wird als Bild kategorisiert.
  - [ ] Medien-URLs werden mit `Range`-loser GET-Anfrage geprüft, der Body aber nach den ersten 64 KB abgebrochen (OS-Festlegung, spart Bandbreite); `Größe` stammt dann aus `Content-Length`.
- **Testfixture:** `crawl-resources`: `/media.html` mit Video (zwei `<source>`), Audio, Track, Poster; `video.mp4` 5 MB.

### CRAWL-043 SWF
- **Priorität:** P2
- **Beschreibung:** Flash-Dateien aus `<object data>`, `<embed src>`, `<param name="movie">`.
- **Eingaben/Konfiguration:** Ressourcenlinks › SWF: Speichern/Crawlen (Standard: an/an).
- **Ausgabe:** Filter „Flash“.
- **Akzeptanzkriterien:**
  - [ ] Alle drei Einbindungsarten werden gefunden.
- **Testfixture:** `crawl-resources/flash.html`.

### CRAWL-044 Interne Hyperlinks
- **Priorität:** P0
- **Beschreibung:** Links aus `<a href>` und `<area href>` auf interne URLs.
- **Eingaben/Konfiguration:** Seitenlinks › Interne Hyperlinks: Speichern/Crawlen (Standard: an/an).
- **Ausgabe:** Tab „Intern“; Inlinks/Outlinks.
- **Akzeptanzkriterien:**
  - [ ] Crawlen aus: nur die Start-URL (und ggf. Ressourcen) wird angefragt.
  - [ ] `<area>` in `<map>` wird wie `<a>` behandelt.
  - [ ] `<a>` ohne `href` oder mit `href="#"` erzeugt keine neue URL.
  - [ ] Mehrfachlinks auf dieselbe Ziel-URL von einer Seite werden in Outlinks einzeln gezählt (Spalte `Anzahl Outlinks`) und in `Eindeutige Outlinks` einfach.
- **Testfixture:** `crawl-links-basic`: `/` mit 3 Links auf `/a` (verschiedene Ankertexte), `<area>`, `<a>` ohne `href`.

### CRAWL-045 Externe Links
- **Priorität:** P0
- **Beschreibung:** Links auf URLs außerhalb des Bereichs. Ist „Crawlen“ an, wird jede externe URL einmal abgerufen (Status). Die Einstellung gilt auch für externe Bilder, CSS, JS, Hreflang- und Canonical-Ziele.
- **Eingaben/Konfiguration:** Seitenlinks › Externe Links: Speichern/Crawlen (Standard: an/an).
- **Ausgabe:** Tab „Extern“.
- **Akzeptanzkriterien:**
  - [ ] Crawlen aus: keine einzige Anfrage an externe Hosts (Request-Log der externen Hosts leer); externe URLs erscheinen ohne Statuscode.
  - [ ] Externe Seiten werden nie geparst, ihre Links nie verfolgt (CRAWL-006).
  - [ ] Externe URLs respektieren deren eigene robots.txt nur, wenn „Externe blockierte URLs anzeigen“ aktiv ist bzw. robots.txt respektiert wird (CRAWL-101).
- **Testfixture:** `crawl-scope-basic` (externer Host mit Link-Kette).

### CRAWL-046 Canonicals
- **Priorität:** P0
- **Beschreibung:** Canonical-Ziele aus `<link rel="canonical">` und HTTP-Header `Link: <…>; rel="canonical"` werden erfasst; wird „Crawlen“ aktiviert, werden die Ziele auch abgerufen und zur Entdeckung genutzt. Die Auswertung (Filter des Canonical-Tabs) ist in einem anderen Bereich spezifiziert.
- **Eingaben/Konfiguration:** Seitenlinks › Canonicals: Speichern/Crawlen (Standard: an/an).
- **Ausgabe:** Spalten `Canonical-Link-Element 1`, `Canonical-Link-Element 2`, `HTTP-Canonical 1`, …
- **Akzeptanzkriterien:**
  - [ ] Crawlen aus: Canonical-Ziel, das nur über das Canonical-Element erreichbar ist, wird nicht angefragt, aber in der Spalte geführt.
  - [ ] `rel="Canonical"` (Großbuchstaben) und `rel="canonical nofollow"` (mehrere Tokens) werden erkannt.
  - [ ] Canonical im `<body>` wird erfasst und als „außerhalb von head“ markiert (für den Canonical-Bereich).
  - [ ] Mehrere Canonical-Elemente werden alle erfasst.
- **Testfixture:** `crawl-canonicals`: `/c1` (canonical auf `/nur-canonical`), `/c2` (Header-Canonical), `/c3` (zwei Elemente), `/c4` (im body).

### CRAWL-047 Paginierung (rel next/prev)
- **Priorität:** P1
- **Beschreibung:** `<link rel="next">`/`<link rel="prev">` sowie entsprechende HTTP-Link-Header.
- **Eingaben/Konfiguration:** Seitenlinks › Paginierung: Speichern (OS-Default: an; die Referenz-Doku sagt nur, dass rel=next/prev standardmäßig weder gecrawlt noch zur Entdeckung genutzt werden, über das Speichern schweigt sie – mögliche Lesart „beides aus“, siehe Q28), Crawlen (Referenz-Default: aus). Begründung des OS-Defaults: Ohne Speichern bleiben die Paginierungsfilter (anderer Bereich) leer.
- **Ausgabe:** Spalten `rel="next" 1`, `rel="prev" 1`, `HTTP rel="next" 1`, `HTTP rel="prev" 1`.
- **Akzeptanzkriterien:**
  - [ ] Standard: `/seite/3`, nur per `rel=next` von `/seite/2` erreichbar, wird nicht angefragt.
  - [ ] Crawlen an: `/seite/3` wird angefragt.
  - [ ] `<a rel="next">` ist ein normaler Hyperlink, kein Paginierungs-Element.
- **Testfixture:** `crawl-pagination`: `/seite/1`…`/seite/5`, verbunden nur per `<link rel=next/prev>`, plus `/seite/1` mit `<a rel="next" href="/seite/2">` als Kontrollfall.

### CRAWL-048 Hreflang
- **Priorität:** P1
- **Beschreibung:** Hreflang-Annotationen aus `<link rel="alternate" hreflang>`, HTTP-Link-Header und XML-Sitemaps (`xhtml:link`) werden extrahiert. Die Ziele werden nur abgerufen, wenn „Hreflang crawlen“ an ist.
- **Eingaben/Konfiguration:** Seitenlinks › Hreflang: Speichern (Standard: an), Crawlen (Standard: aus).
- **Ausgabe:** Spalten `Hreflang-Sprache n`, `Hreflang-URL n`; Hreflang-Tab (anderer Bereich).
- **Akzeptanzkriterien:**
  - [ ] Speichern an, Crawlen aus: Alternativ-URL nur über Hreflang erreichbar → nicht angefragt, aber in den Spalten gelistet.
  - [ ] Crawlen an: Alternativ-URL wird angefragt, auch auf anderer Subdomain/ccTLD (als extern, sofern nicht im Bereich).
  - [ ] Hreflang aus Sitemaps wird nur gelesen, wenn Sitemaps gecrawlt werden (CRAWL-059).
- **Testfixture:** `crawl-hreflang`: `/de/` mit Alternates auf `/en/`, `/fr/` (nur per Hreflang erreichbar), `x-default`, HTTP-Header-Variante auf `/header-hreflang`.

### CRAWL-049 AMP
- **Priorität:** P2
- **Beschreibung:** AMP-Varianten aus `<link rel="amphtml">`.
- **Eingaben/Konfiguration:** Seitenlinks › AMP: Speichern (Standard: aus), Crawlen (Standard: aus).
- **Ausgabe:** Spalte `amphtml-Link-Element`; AMP-Tab (anderer Bereich).
- **Akzeptanzkriterien:**
  - [ ] Standard: AMP-URL wird weder angefragt noch gespeichert.
  - [ ] Beide an: AMP-URL wird angefragt und als AMP-Seite markiert.
- **Testfixture:** `crawl-amp`: `/artikel` mit `amphtml` → `/artikel/amp`.

### CRAWL-050 Meta Refresh und iframes
- **Priorität:** P1
- **Beschreibung:** Ziele aus Meta-Refresh (CRAWL-015) und Quellen aus `<iframe src>`/`<frame src>`.
- **Eingaben/Konfiguration:** Seitenlinks › Meta Refresh: Speichern/Crawlen (Standard: an/an); iframes: Speichern/Crawlen (Standard: an/an).
- **Ausgabe:** Inlinks mit `Linktyp` „Meta Refresh“ bzw. „iframe“.
- **Akzeptanzkriterien:**
  - [ ] iframe-Ziel `/eingebettet` wird angefragt; Inlink-Typ „iframe“.
  - [ ] `<iframe srcdoc>` erzeugt keine URL.
  - [ ] Crawlen aus: iframe-Ziel nicht angefragt.
- **Testfixture:** `crawl-frames`: `/mit-iframe`, `/frameset` (`<frame>`), `/srcdoc`.

### CRAWL-051 Mobile-Alternate-Links
- **Priorität:** P2
- **Beschreibung:** Separate Mobilversionen aus `<link rel="alternate" media="only screen and (max-width: 640px)">`.
- **Eingaben/Konfiguration:** Seitenlinks › Mobile Alternate: Speichern/Crawlen (Standard: aus/aus).
- **Ausgabe:** Spalte `Mobile-Alternate-Link`.
- **Akzeptanzkriterien:**
  - [ ] Nur `rel="alternate"` mit `media`-Attribut zählt; `hreflang`-Alternates werden nicht verwechselt.
  - [ ] Standard: Mobil-URL nicht angefragt.
- **Testfixture:** `crawl-mobile-alt`: `www.site.test/` mit Alternate auf `m.site.test/`.

### CRAWL-052 Link-Quellen im HTML (welche Elemente Links erzeugen)
- **Priorität:** P0
- **Beschreibung:** Verbindliche Liste, aus welchen HTML-Elementen und Attributen URLs entnommen werden, und welcher `Linktyp` vergeben wird. Alle Elemente werden auch in Groß-/Mischschreibung erkannt; Parser ist HTML5-konform (fehlerhaftes Markup wie ein Browser).
- **Eingaben/Konfiguration:** Schalter aus CRAWL-039–051.
- **Ausgabe:** Inlinks/Outlinks mit Spalten `Typ` (Hyperlink, Bild, CSS, JavaScript, Medien, Canonical, Hreflang, Next/Prev, AMP, Meta Refresh, iframe, Mobile Alternate, Sonstige), `Quelle`, `Ziel`, `Alt-Text`, `Ankertext`, `Folgen` (true/false), `Rel`, `Ziel-Attribut` (`target`), `Pfadtyp` (absolut, relativ zum Stamm, relativ zum Pfad, protokollrelativ), `Link-Pfad` (XPath), `Link-Position` (Kopf/Navigation/Inhalt/Seitenleiste/Fuß – Logik im Bereich Links), `Link-Herkunft` (CRAWL-079).
- **Akzeptanzkriterien:**
  - [ ] Hyperlinks: `a[href]`, `area[href]`.
  - [ ] Bilder: `img[src]`, `img[srcset]`*, `picture source[srcset]`*, `input[type=image][src]`, `video[poster]` (*nur mit CRAWL-055).
  - [ ] CSS: `link[rel~=stylesheet][href]`; JS: `script[src]`, `link[rel=modulepreload]`.
  - [ ] Medien: `video[src]`, `audio[src]`, `source[src]`, `track[src]`; Flash: `object[data]`, `embed[src]`.
  - [ ] Frames: `iframe[src]`, `frame[src]`.
  - [ ] `link[rel=preload|prefetch][href]` → Typ nach `as`-Attribut, sonst „Sonstige“ (OS-Festlegung).
  - [ ] Links in HTML-Kommentaren, `<template>`, `<noscript>` (im Textmodus: `<noscript>` wird geparst, OS-Festlegung siehe Q) und `<script>`-Inhalten werden nicht extrahiert.
  - [ ] Der Ankertext ist der normalisierte Textinhalt des `<a>`; bei Bildlinks ohne Text wird der `alt` des Bildes in `Alt-Text` geführt.
  - [ ] `Folgen` = false, wenn `rel` eines der Tokens `nofollow`, `sponsored`, `ugc` enthält oder die Seite meta-/header-seitig nofollow ist.
- **Testfixture:** `crawl-link-sources`: eine Seite mit je einem Element jeder Zeile oben, Großschreibung (`<A HREF>`), kaputtem Markup (nicht geschlossene Tags), Link im Kommentar, in `<template>`, in `<noscript>`, `rel="sponsored"`, `rel="ugc"`, `rel="nofollow noopener"`.

### CRAWL-053 Nicht crawlbare Links speichern
- **Priorität:** P1
- **Beschreibung:** Elemente, die wie Links wirken, aber keinen von Suchmaschinen verfolgbaren Link darstellen, werden optional gesammelt und gemeldet. Sie werden nie angefragt.
- **Eingaben/Konfiguration:** Spider › Crawl › „Nicht crawlbare Links“ › Speichern (Standard: aus).
- **Ausgabe:** Outlinks-Spalte `Link-Crawlbarkeit` (`Crawlbar` / `Nicht crawlbar`); Filter im Links-Tab „Seiten mit nicht crawlbaren internen Outlinks“; Sammelexport „Links › Seiten mit nicht crawlbaren internen Outlinks“.
- **Akzeptanzkriterien:**
  - [ ] Als nicht crawlbar gelten: `span[href]`, `div[href]`, jedes andere Nicht-`a`/`area`-Element mit `href`; `a` ohne `href`, aber mit `onclick`, das Navigation auslöst (`location`/`window.open` im Handler-Text); `a[href^="javascript:"]`; `a` mit `href` und `onclick`, das `return false` enthält oder navigiert (OS-Heuristik).
  - [ ] Ziele werden, soweit aus dem Attribut ableitbar, aufgelöst und angezeigt; sonst bleibt `Ziel` leer und das Roh-Attribut steht in `Link-Pfad`.
  - [ ] Option aus: keine solchen Einträge, Filter leer.
  - [ ] Kein Request an die Ziele, auch wenn sie sonst intern wären.
- **Testfixture:** `crawl-uncrawlable`: `/` mit `<span href="/s">`, `<div href="/d">`, `<a onclick="location.href='/o'">`, `<a href="javascript:go('/j')">`, Kontrolllink `<a href="/ok">`.

### CRAWL-054 Ungültige Links crawlen
- **Priorität:** P2
- **Beschreibung:** Syntaktisch ungültige Link-Ziele (z. B. Tippfehler im Schema wie `hppts://`, Leerzeichen im Host) werden standardmäßig verworfen. Optional werden sie geparst, gemeldet und – soweit möglich – angefragt.
- **Eingaben/Konfiguration:** Spider › Crawl › „Ungültige Links crawlen“ (Standard: aus).
- **Ausgabe:** Zeilen mit Status „Ungültige URL“ unter „Antwortcodes › Keine Antwort“.
- **Akzeptanzkriterien:**
  - [ ] Option aus: `hppts://site.test/x` erscheint nirgends.
  - [ ] Option an: Eintrag mit Statuscode 0, Status „Ungültige URL“, Inlink von der Quellseite.
  - [ ] `http://site test/` (Leerzeichen im Host) und `http://:80/` werden ebenso behandelt.
  - [ ] Ein relatives `hppts://…` wird nicht als relativer Pfad interpretiert.
- **Testfixture:** `crawl-invalid-links`: `/` mit `hppts://site.test/x`, `http://site test/`, `http://:80/`, `http//site.test/fehlt-doppelpunkt` (wird als relativer Pfad aufgelöst – Kontrollfall).

### CRAWL-055 Bilder aus srcset extrahieren
- **Priorität:** P1
- **Beschreibung:** Neben `src` werden alle Kandidaten aus `srcset` (an `<img>` und `<picture><source>`) als Bilder erfasst.
- **Eingaben/Konfiguration:** Erweitert › „Bilder aus img srcset extrahieren“ (Standard: aus; OS-Default, siehe Q).
- **Ausgabe:** zusätzliche Bild-URLs mit Inlink-Typ „Bild“.
- **Akzeptanzkriterien:**
  - [ ] `srcset="a.jpg 1x, b.jpg 2x"` → `a.jpg`, `b.jpg`.
  - [ ] `srcset="a.jpg 480w, b.jpg 800w"` → beide; URLs mit Kommas in Query (`img.jpg?x=1,2 2x`) werden korrekt getrennt (Parser nach HTML-Spezifikation).
  - [ ] Option aus: nur `src`.
- **Testfixture:** `crawl-srcset`: `/` mit beiden Formaten, `<picture>` mit zwei `<source>`, Komma-in-URL-Fall.

### CRAWL-056 Nofollow-Links folgen (intern/extern)
- **Priorität:** P0
- **Beschreibung:** Links mit `rel` nofollow/sponsored/ugc sowie alle Links einer Seite mit Meta-Robots- oder X-Robots-Tag-`nofollow` werden standardmäßig gespeichert, aber nicht verfolgt. Getrennte Optionen für intern und extern heben das auf.
- **Eingaben/Konfiguration:** Spider › Crawl › „Internem nofollow folgen“ (Standard: aus), „Externem nofollow folgen“ (Standard: aus).
- **Ausgabe:** Outlinks-Spalte `Folgen` = false; URLs, die ausschließlich über nofollow-Links erreichbar sind, fehlen in der Crawl-Tabelle.
- **Akzeptanzkriterien:**
  - [ ] `/nur-nofollow` (einziger Link mit `rel="nofollow"`) wird standardmäßig nicht angefragt.
  - [ ] Wird dieselbe URL zusätzlich irgendwo per Follow-Link verlinkt, wird sie angefragt.
  - [ ] Seite mit `<meta name="robots" content="nofollow">`: keiner ihrer Links wird verfolgt; ebenso mit Header `X-Robots-Tag: nofollow`.
  - [ ] User-Agent-spezifische Direktiven (`<meta name="googlebot" content="nofollow">`, `X-Robots-Tag: googlebot: nofollow`) gelten, wenn der eingestellte robots-User-Agent passt (CRAWL-114); `name="robots"` gilt immer.
  - [ ] „Internem nofollow folgen“ an: interne nofollow-Ziele werden angefragt, externe nicht (und umgekehrt).
  - [ ] `rel="sponsored"` und `rel="ugc"` verhalten sich wie `nofollow`.
- **Testfixture:** `crawl-nofollow`: `/` mit `rel=nofollow`, `rel=sponsored`, `rel=ugc` auf interne und externe Ziele; `/meta-nofollow`, `/header-nofollow`, `/googlebot-nofollow`.

### CRAWL-057 Links in PDF-Dokumenten
- **Priorität:** P1
- **Beschreibung:** Gecrawlte PDFs werden geparst: Link-Annotationen (URI-Aktionen) werden als Outlinks erfasst, der Dokumenttitel wird als Seitentitel verwendet.
- **Eingaben/Konfiguration:** greift, wenn PDFs gecrawlt werden (sie sind HTML-Links bzw. Ressourcen); Extraktion › PDF (CRAWL-064).
- **Ausgabe:** Outlinks einer PDF mit `Ankertext` (Text im Link-Rechteck, falls extrahierbar) und `PDF-Seite` (Seitennummer).
- **Akzeptanzkriterien:**
  - [ ] Interne Links aus der PDF werden gecrawlt; externe geprüft.
  - [ ] Nur absolute URLs aus URI-Aktionen werden verwendet; relative oder dateilokale Verweise werden ignoriert.
  - [ ] URLs mit Leerzeichen/Zeilenumbrüchen innerhalb der Annotation werden bereinigt.
  - [ ] Verschlüsselte oder defekte PDFs führen zu keinem Absturz; Hinweis in Spalte `Status`-Detail.
  - [ ] Titel aus PDF-Metadaten `Title`; fehlt er, bleibt der Titel leer.
- **Testfixture:** `crawl-pdf`: `/doc.pdf` (3 Seiten, 2 interne + 1 externer Link, Titel „Testdokument“), `/broken.pdf`, `/encrypted.pdf`.

---
## D. XML-Sitemaps

### CRAWL-058 Sitemap-Erkennung über robots.txt
- **Priorität:** P1
- **Beschreibung:** `Sitemap:`-Zeilen der robots.txt jeder gecrawlten Subdomain werden ausgelesen und die dort genannten Sitemaps automatisch abgerufen.
- **Eingaben/Konfiguration:** Spider › Crawl › XML-Sitemaps: „Verlinkte XML-Sitemaps crawlen“ (Standard: aus); darunter „XML-Sitemaps automatisch über robots.txt finden“ (Standard: an, nur wirksam, wenn die übergeordnete Option an ist).
- **Ausgabe:** Sitemap-URLs im Tab „Intern › XML“ (bzw. Sitemaps-Tab, anderer Bereich); jede in Sitemaps gefundene URL erhält die Zuordnung `In Sitemap` = ja und die Liste der Sitemaps, in denen sie steht.
- **Akzeptanzkriterien:**
  - [ ] `Sitemap:` wird case-insensitiv erkannt, unabhängig von seiner Position in der Datei und von User-Agent-Gruppen.
  - [ ] Relative Sitemap-Angaben werden gegen die robots.txt-URL aufgelöst (OS-Festlegung; laut Standard sollen sie absolut sein).
  - [ ] Sitemaps auf einem anderen Host werden ebenfalls abgerufen, ihre Einträge aber nur übernommen, wenn sie im Crawl-Bereich liegen.
  - [ ] Option aus: robots.txt-Sitemaps werden nicht abgerufen (Request-Log).
  - [ ] Wird robots.txt ignoriert (CRAWL-099 „Ignorieren“), findet keine Autodiscovery statt.
- **Testfixture:** `crawl-sitemaps`: `robots.txt` mit `Sitemap: /sitemap_index.xml` und `sitemap: https://site.test/news.xml`; Index → `pages.xml`, `pages2.xml.gz`; `pages.xml` enthält eine verwaiste URL `/nur-in-sitemap` und eine externe URL.

### CRAWL-059 Verlinkte und vorgegebene Sitemaps crawlen
- **Priorität:** P1
- **Beschreibung:** Zusätzlich zur Autodiscovery können Sitemap-URLs manuell vorgegeben werden. Die darin enthaltenen URLs werden in die Warteschlange aufgenommen, sodass auch nicht verlinkte (verwaiste) Seiten gecrawlt werden. Die Auswertung (Filter „Nicht in Sitemap“, „Verwaist“ usw.) erfolgt in der Crawl-Analyse (Nachlauf, CRAWL-141 / UI-080); bis sie gelaufen ist, zeigen diese Filter den Hinweis „Crawl-Analyse erforderlich“.
- **Eingaben/Konfiguration:** Spider › Crawl › XML-Sitemaps: „Diese Sitemaps crawlen“ (Textfeld, eine URL pro Zeile).
- **Ausgabe:** wie CRAWL-058.
- **Akzeptanzkriterien:**
  - [ ] `/nur-in-sitemap` wird gecrawlt, hat `Crawl-Tiefe` leer und keine Inlinks aus HTML.
  - [ ] Sitemap-URLs werden nach allen über Links erreichbaren URLs eingereiht (CRAWL-005), damit Tiefenwerte nicht verfälscht werden.
  - [ ] Sitemap-Einträge unterliegen Include/Exclude, robots.txt, URL-Umschreibung und Limits wie normale URLs.
  - [ ] Doppelte Angabe derselben Sitemap (manuell + robots.txt) führt zu genau einem Abruf.
  - [ ] Eine spät entdeckte Sitemap (z. B. erst aus einer Kind-Sitemap) blockiert den Crawl nicht (Referenz-Fix 20.4).
- **Testfixture:** `crawl-sitemaps` (manuelle Angabe `/extra-sitemap.xml`, nicht in robots.txt).

### CRAWL-060 Sitemap-Parser
- **Priorität:** P1
- **Beschreibung:** Robuster Parser für XML-Sitemaps und Sitemap-Indizes nach sitemaps.org-Protokoll inklusive gängiger Erweiterungen.
- **Eingaben/Konfiguration:** keine.
- **Ausgabe:** je Eintrag: `loc`, `lastmod`, `changefreq`, `priority`; Erweiterungen: `image:loc`, `xhtml:link rel=alternate hreflang` (für Hreflang-Bereich), `video:*`, `news:*` (nur gespeichert, P2-Auswertung).
- **Akzeptanzkriterien:**
  - [ ] `<urlset>` und `<sitemapindex>` werden erkannt; verschachtelte Indizes (Index in Index) werden bis Tiefe 5 verfolgt (OS-Default), darüber Warnung.
  - [ ] gzip-Dateien werden anhand Magic-Bytes erkannt, unabhängig von Endung und Content-Type.
  - [ ] Whitespace um `loc` wird entfernt; XML-Entities (`&amp;`) werden dekodiert.
  - [ ] Ungültiges XML: alle bis zum Fehler gelesenen Einträge werden verwendet; Fehler wird an der Sitemap vermerkt.
  - [ ] Sitemaps über 50 000 URLs bzw. 50 MB (unkomprimiert) werden vollständig gelesen, aber für den Sitemaps-Bereich als Verstoß markiert.
  - [ ] Plain-Text-Sitemaps (eine URL pro Zeile) und RSS/Atom-Feeds werden als Sitemap akzeptiert (OS-Festlegung, siehe Q).
  - [ ] Speicherverbrauch beim Parsen einer 50-MB-Sitemap bleibt unter 100 MB (Streaming-Parser).
- **Testfixture:** `crawl-sitemaps`: `index-nested.xml` (3 Ebenen), `big.xml` (50 001 Einträge, generiert), `broken.xml` (Abbruch nach 10 Einträgen), `entities.xml`, `sitemap.txt`, `feed.rss`, `image-sitemap.xml`, `hreflang-sitemap.xml`.

---

## E. Extraktion (crawl-seitige Schalter)

Dieser Abschnitt legt nur fest, **dass** und **wann** die Engine Daten erhebt und speichert. Die fachliche Auswertung (Filter, Grenzwerte) steht in den Spezifikationen der jeweiligen Tabs.

### CRAWL-061 Schalter für Seitendetails
- **Priorität:** P1
- **Beschreibung:** Einzelne Extraktionen können abgeschaltet werden, um Speicher und Zeit zu sparen. Abgeschaltete Elemente verschwinden aus der Oberfläche; davon abhängige Filter werden ausgeblendet.
- **Eingaben/Konfiguration:** Spider › Extraktion › Seitendetails: Seitentitel, Meta-Beschreibung, Meta-Keywords, H1, H2, Indexierbarkeit, Wortanzahl, Lesbarkeit, Text-zu-Code-Verhältnis, Hash-Wert, Seitengröße, Formulare, Barrierefreiheit (erfordert JS-Rendering). Standard: alle an außer Barrierefreiheit (OS-Default, siehe Q).
- **Ausgabe:** entsprechende Spalten/Filter vorhanden oder nicht.
- **Akzeptanzkriterien:**
  - [ ] „Hash-Wert“ aus: Spalte `Hash` fehlt; Filter „Exakte Duplikate“ ist ausgeblendet.
  - [ ] „Seitentitel“ aus: Tab „Seitentitel“ zeigt einen Hinweis „Extraktion deaktiviert“ statt leerer Filter.
  - [ ] „Barrierefreiheit“ an ohne JS-Rendering: Konfigurationsdialog warnt und bietet an, Rendering zu aktivieren.
  - [ ] Abschalten während eines pausierten Crawls wirkt nur auf noch nicht gecrawlte URLs; bereits erhobene Daten bleiben.
- **Testfixture:** `crawl-site-100` mit Profilen „alles an“ / „Hash aus“ / „Titel aus“.

### CRAWL-062 URL-Details: Antwortzeit, Last-Modified, HTTP-Header, Cookies
- **Priorität:** P1
- **Beschreibung:** Optional werden alle Response-Header und alle gesetzten Cookies je URL gespeichert.
- **Eingaben/Konfiguration:** Spider › Extraktion › URL-Details: Antwortzeit (OS-Default: an), Last-Modified (OS-Default: an), HTTP-Header (OS-Default: aus), Cookies (OS-Default: aus). Die Referenz-Doku beschreibt die Optionen, nennt aber keine Standardwerte (siehe Q29).
- **Ausgabe:** Detailbereich „HTTP-Header“ (Request- und Response-Header als Liste); Detailbereich „Cookies“ (Name, Wert, Domain, Pfad, Ablauf, Secure, HttpOnly, SameSite, Herkunft HTTP/JS); bei HTTP-Header-Speicherung zusätzlich je Header-Name eine Spalte im Tab „Intern“ (z. B. `Header: Cache-Control`).
- **Akzeptanzkriterien:**
  - [ ] Mehrfach-Header (z. B. zwei `Set-Cookie`, zwei `Link`) werden einzeln gespeichert, Reihenfolge erhalten.
  - [ ] Cookies, die per JavaScript gesetzt werden, erscheinen nur bei JS-Rendering (Herkunft „JS“).
  - [ ] Ist Cookie-Speicherung an, werden Analytics-Skripte beim Rendern nicht mehr automatisch blockiert (sonst fehlen deren Cookies). Standard (aus): bekannte Analytics-/Tag-Manager-Endpunkte werden beim Rendern blockiert, damit Crawls die Statistik der Website nicht verfälschen (OS-Liste: google-analytics.com, googletagmanager.com, analytics.google.com; erweiterbar).
  - [ ] Header-Spalten erscheinen dynamisch für jeden beobachteten Header-Namen (case-insensitiv zusammengeführt).
- **Testfixture:** `crawl-headers-cookies`: `/set-cookies` (drei `Set-Cookie`, eines mit allen Attributen), `/js-cookie` (setzt `document.cookie`), `/many-headers`, `/ga.html` (lädt Fake-Analytics-Skript von `www.google-analytics.test`, über Host-Überschreibung auf Fixture gemappt).

### CRAWL-063 HTML und gerendertes HTML speichern
- **Priorität:** P1
- **Beschreibung:** Rohes HTML und (bei JS-Rendering) das serialisierte DOM nach dem Rendern werden pro URL gespeichert, um sie anzuzeigen, zu vergleichen und massenhaft zu exportieren.
- **Eingaben/Konfiguration:** Spider › Extraktion: „HTML speichern“ (Standard: aus), „Gerendertes HTML speichern“ (Standard: aus; nur mit JS-Rendering wählbar).
- **Ausgabe:** Detailbereich „Quelltext“ mit Ansichten Roh/Gerendert/Diff; Sammelexport aller HTML-Dateien.
- **Akzeptanzkriterien:**
  - [ ] Gespeichertes HTML ist byte-identisch zum dekomprimierten Response-Body (vor Zeichensatz-Konvertierung, Zeichensatz wird mitgespeichert).
  - [ ] Speicherung erfolgt komprimiert (zstd) in der Crawl-Datei; ein Crawl mit 1 000 Seiten à 100 KB wächst um < 40 MB.
  - [ ] Ohne Speicherung ist der Quelltext-Bereich leer mit Hinweis, wie man ihn aktiviert.
  - [ ] Nur HTML-Inhalte werden gespeichert (keine Bilder/CSS/JS).
- **Testfixture:** `crawl-site-100`.

### CRAWL-064 PDF speichern und Eigenschaften extrahieren
- **Priorität:** P2
- **Beschreibung:** PDFs können gespeichert und ihre Metadaten extrahiert werden.
- **Eingaben/Konfiguration:** Spider › Extraktion › PDF: „PDF speichern“ (aus), „PDF-Eigenschaften extrahieren“ (aus; Titel und Schlüsselwörter werden immer gelesen), „Linktext aus PDF extrahieren“ (aus; speicher-/zeitintensiv).
- **Ausgabe:** Spalten `PDF-Titel`, `PDF-Thema`, `PDF-Autor`, `PDF-Erstellt`, `PDF-Geändert`, `PDF-Seitenzahl`, `PDF-Wortanzahl`, `PDF-Schlüsselwörter`.
- **Akzeptanzkriterien:**
  - [ ] Eigenschaften aus: nur `PDF-Titel` und `PDF-Schlüsselwörter` befüllt.
  - [ ] Eigenschaften an: alle Spalten befüllt (Fixture-Werte exakt).
  - [ ] Linktext aus: Ankertexte von PDF-Outlinks sind leer.
- **Testfixture:** `crawl-pdf/doc.pdf` mit allen Metadaten.

### CRAWL-065 Direktiven- und Strukturdaten-Schalter
- **Priorität:** P1
- **Beschreibung:** Erhebung von Meta-Robots, X-Robots-Tag und strukturierten Daten ist schaltbar. Auswertung siehe jeweilige Bereiche.
- **Eingaben/Konfiguration:** Spider › Extraktion › Direktiven: Meta Robots (OS-Default: an), X-Robots-Tag (OS-Default: an). Strukturierte Daten: JSON-LD (OS-Default: aus), Microdata (aus), RDFa (aus), Schema.org-Validierung (aus), Google-Rich-Result-Validierung (aus), Groß-/Kleinschreibung beachten (aus). Sämtliche Standardwerte dieses Eintrags sind OS-Defaults (in der Referenz-Doku nicht genannt, siehe Q29).
- **Ausgabe:** abhängige Spalten/Tabs.
- **Akzeptanzkriterien:**
  - [ ] X-Robots-Tag aus: ein `X-Robots-Tag: noindex` beeinflusst die Indexierbarkeit nicht und `nofollow` im Header verhindert das Folgen nicht.
  - [ ] Meta Robots aus: analog für das Meta-Tag.
  - [ ] Validierungsoptionen sind nur wählbar, wenn mindestens ein Format aktiviert ist.
- **Testfixture:** `crawl-nofollow/header-nofollow`, `crawl-nofollow/meta-nofollow`.

---

## F. Limits

Gemeinsame Regeln: Ein Limit von 0 oder leer bedeutet „unbegrenzt“, sofern nicht anders angegeben. Durch Limits nicht gecrawlte URLs erscheinen nicht in den Tabellen, bleiben aber als Outlinks der Quellseite sichtbar (ohne Statuscode). Die Statusleiste zeigt am Ende, wie viele URLs je Limit übersprungen wurden (Bericht „Crawl-Übersicht“, Abschnitt „Durch Limits übersprungen“; OS-Erweiterung).

### CRAWL-066 Maximale Gesamtzahl URLs
- **Priorität:** P0
- **Beschreibung:** Obergrenze für die Anzahl gecrawlter URLs (angefragte URLs, inklusive Ressourcen und externer URLs).
- **Eingaben/Konfiguration:** Limits › „Gesamtzahl begrenzen“ (Standard: 5 000 000; Referenz-Default).
- **Ausgabe:** Crawl endet mit Status „Limit erreicht“.
- **Akzeptanzkriterien:**
  - [ ] Limit 10 bei Site mit 100 Seiten: genau 10 Requests (Request-Log), danach Ende; keine weiteren Requests durch noch laufende Threads.
  - [ ] Weiterleitungsziele und Ressourcen zählen mit.
  - [ ] Die robots.txt-Abrufe zählen nicht mit.
  - [ ] Wird das Limit nach Pause erhöht, setzt „Fortsetzen“ den Crawl fort.
- **Testfixture:** `crawl-site-100`.

### CRAWL-067 Maximale Crawl-Tiefe
- **Priorität:** P0
- **Beschreibung:** URLs mit größerer Crawl-Tiefe als der Grenzwert werden nicht angefragt.
- **Eingaben/Konfiguration:** Limits › „Crawl-Tiefe begrenzen“ (Standard: unbegrenzt im Spider-Modus, 0 im Listenmodus).
- **Ausgabe:** —
- **Akzeptanzkriterien:**
  - [ ] Tiefe 2 bei Kette `/`→`/1`→`/2`→`/3`: `/`, `/1`, `/2` werden angefragt, `/3` nicht; `/3` erscheint als Outlink von `/2`.
  - [ ] Tiefe 0: nur die Start-URL.
  - [ ] Ressourcen der Seiten auf maximaler Tiefe werden trotzdem geprüft (OS-Festlegung, damit Bild-/CSS-Status der letzten Ebene vorhanden ist; siehe Q).
  - [ ] Bei „Weiterleitungen immer folgen“ werden Weiterleitungsziele unabhängig von der Tiefe verfolgt (CRAWL-030).
- **Testfixture:** `crawl-depth`.

### CRAWL-068 URLs pro Crawl-Tiefe
- **Priorität:** P2
- **Beschreibung:** Begrenzt die Anzahl angefragter URLs je Tiefenstufe – sinnvoll für Stichproben großer Sites.
- **Eingaben/Konfiguration:** Limits › „URLs pro Crawl-Tiefe begrenzen“ (Standard: aus).
- **Akzeptanzkriterien:**
  - [ ] Wert 5 bei Startseite mit 30 Kindlinks: auf Tiefe 1 werden genau 5 URLs angefragt (die ersten 5 in Dokumentreihenfolge).
  - [ ] Die Grenze gilt je Tiefe unabhängig (Tiefe 2 erhält wieder bis zu 5).
- **Testfixture:** `crawl-depth` (Seite mit 30 Kindern).

### CRAWL-069 Maximale Ordnertiefe
- **Priorität:** P1
- **Beschreibung:** Begrenzt nach Anzahl Verzeichnisebenen im URL-Pfad. Ordner sind Pfadsegmente, die mit `/` abgeschlossen sind; die Wurzel `/` hat Ordnertiefe 0; ein Dateiname zählt nicht als Ordner.
- **Eingaben/Konfiguration:** Limits › „Max. Ordnertiefe begrenzen“ (Standard: aus).
- **Ausgabe:** Spalte `Ordnertiefe` `folder_depth` (immer berechnet, unabhängig vom Limit).
- **Akzeptanzkriterien:**
  - [ ] `/` → 0; `/a/` → 1; `/a/b/` → 2; `/a/datei.html` → 1; `/a/b/c/index.html` → 3; `/a?x=/y/` → 1 (Query zählt nicht).
  - [ ] Limit 1: `/a/b/` und tiefer werden nicht angefragt; `/a/datei.html` wird angefragt.
- **Testfixture:** `crawl-folders`: `/`, `/a/`, `/a/x.html`, `/a/b/`, `/a/b/c/index.html`.

### CRAWL-070 Maximale Anzahl Query-Parameter
- **Priorität:** P1
- **Beschreibung:** URLs mit mehr als n Query-Parametern werden nicht gecrawlt (Schutz vor Facetten-Explosion).
- **Eingaben/Konfiguration:** Limits › „Anzahl Query-Strings begrenzen“ (Standard: aus).
- **Akzeptanzkriterien:**
  - [ ] Wert 2: `?a=1&b=2` wird gecrawlt, `?a=1&b=2&c=3` nicht.
  - [ ] Gezählt werden durch `&` getrennte nichtleere Paare; `?a=1&&b=2` zählt 2; `?a` zählt 1; `;` gilt nicht als Trenner (OS-Festlegung).
  - [ ] Wert 0: keine URL mit Query wird gecrawlt (Abweichung von der allgemeinen 0-Regel; 0 ist hier ein gültiger Wert, „aus“ ist ein eigener Zustand).
  - [ ] Zählung erfolgt nach URL-Umschreibung (CRAWL-103).
- **Testfixture:** `crawl-facets`: `/kategorie` mit Links auf Kombinationen von 0–4 Filterparametern.

### CRAWL-071 Maximale URLs pro Subdomain
- **Priorität:** P2
- **Beschreibung:** Begrenzt die gecrawlten URLs je Subdomain getrennt, z. B. um im Listenmodus aus vielen Domains je eine Stichprobe zu ziehen.
- **Eingaben/Konfiguration:** Limits › „Gesamtzahl je Subdomain begrenzen“ (Standard: aus).
- **Akzeptanzkriterien:**
  - [ ] Wert 3 mit „Alle Subdomains“: von `www`, `blog`, `shop` werden jeweils höchstens 3 URLs angefragt.
  - [ ] Im Listenmodus mit Tiefe 1 und Liste aus 5 Domains: je Domain höchstens n URLs.
- **Testfixture:** `crawl-scope-subdomains`.

### CRAWL-072 Maximale Anzahl verfolgter Weiterleitungen
- **Priorität:** P0
- **Beschreibung:** Begrenzt die Länge einer verfolgten Weiterleitungskette ab der zuerst entdeckten URL.
- **Eingaben/Konfiguration:** Limits › „Max. Weiterleitungen“ (OS-Default: 10, angelehnt an Googlebot; Referenz-Default nicht dokumentiert, siehe Q).
- **Ausgabe:** letzte nicht mehr verfolgte Ziel-URL steht als `Weiterleitungs-URL` der letzten Hop-Zeile; Kette im Bericht mit Hinweis „Limit erreicht“.
- **Akzeptanzkriterien:**
  - [ ] Limit 3, Kette mit 5 Hops: es werden die Start-URL und 3 Ziele angefragt; das 4. Ziel nicht.
  - [ ] Die Zählung beginnt pro Kette neu, nicht pro Crawl.
  - [ ] Erreicht eine Kette das Limit, erscheint sie zusätzlich im Filter „Weiterleitungskette“.
- **Testfixture:** `crawl-redirect-chains`: `/hop1`…`/hop6`.

### CRAWL-073 Maximale URL-Länge
- **Priorität:** P1
- **Beschreibung:** URLs, deren Zeichenzahl (nach Normalisierung, inkl. Schema und Host) den Grenzwert überschreitet, werden nicht gecrawlt.
- **Eingaben/Konfiguration:** Limits › „Max. URL-Länge“ (Standard: 10 000 Zeichen; Referenz-Default).
- **Akzeptanzkriterien:**
  - [ ] Limit 100: URL mit 100 Zeichen wird gecrawlt, mit 101 nicht.
  - [ ] Gezählt wird die prozentkodierte Form.
  - [ ] Unabhängig davon meldet der URL-Tab überlange URLs ab 115 Zeichen (Grenzwert dort konfigurierbar; anderer Bereich).
- **Testfixture:** `crawl-url-length`: generierte Links mit 99/100/101/10 001 Zeichen.

### CRAWL-074 Maximale Links pro URL
- **Priorität:** P1
- **Beschreibung:** Pro Seite werden höchstens n Links extrahiert; weitere werden verworfen (Schutz vor Linkfarmen/Endlos-Kalendern).
- **Eingaben/Konfiguration:** Limits › „Max. Links pro URL“ (Standard: 10 000; Referenz-Default).
- **Akzeptanzkriterien:**
  - [ ] Seite mit 15 000 Links, Limit 10 000: Outlinks enthalten exakt die ersten 10 000 in Dokumentreihenfolge.
  - [ ] Der Abbruch wird an der Seite vermerkt (Spalte `Links gekappt` = ja; OS-Erweiterung).
  - [ ] Ressourcen-Links (Bilder/CSS/JS) zählen mit (OS-Festlegung, siehe Q).
- **Testfixture:** `crawl-many-links`: `/viele` mit 15 000 generierten Links.

### CRAWL-075 Maximale Seitengröße
- **Priorität:** P1
- **Beschreibung:** Bodies über dem Grenzwert werden nicht weiter heruntergeladen; die Seite wird bis zur Grenze verarbeitet.
- **Eingaben/Konfiguration:** Limits › „Max. Seitengröße (KB)“ (Standard: 51 200 KB = 50 MB; Referenz-Default).
- **Ausgabe:** `Größe` zeigt die tatsächliche Größe aus `Content-Length`, sofern vorhanden, sonst die gelesene; Vermerk `Abgeschnitten` = ja.
- **Akzeptanzkriterien:**
  - [ ] Limit 1 MB, Seite 3 MB: höchstens 1 MB + 64 KB Puffer werden gelesen (Byte-Zähler im Fixture-Server), Links aus dem ersten MB sind vorhanden.
  - [ ] Bei Kompression gilt das Limit für die dekomprimierten Bytes (Schutz vor Zip-Bomben): eine gzip-Bombe (1 MB → 1 GB) wird nach Grenzwert abgebrochen, Speicherverbrauch bleibt begrenzt.
- **Testfixture:** `crawl-big-page`: `/3mb.html` (Links am Anfang und Ende), `/gzip-bomb`.

### CRAWL-076 Limit nach URL-Pfad
- **Priorität:** P2
- **Beschreibung:** Liste aus Regex-Muster + Höchstzahl: Für URLs, die auf ein Muster passen, werden höchstens n gecrawlt – z. B. 50 Produktseiten als Stichprobe.
- **Eingaben/Konfiguration:** Limits › „Nach URL-Pfad begrenzen“: Tabelle (Muster, Max. URLs).
- **Akzeptanzkriterien:**
  - [ ] Muster `.*/produkt/.*` mit Max 5 bei 40 Produktseiten: genau 5 angefragt; andere Bereiche unbeeinflusst.
  - [ ] Abgleich als Vollübereinstimmung (Konventionen): Muster `/produkt/` ohne `.*` begrenzt keine einzige URL; der Konfigurationsdialog zeigt dazu einen Hinweis „Muster passt auf keine vollständige URL – `.*` ergänzen?“, sobald es auf keine bisher gecrawlte URL passt.
  - [ ] Passt eine URL auf mehrere Muster, zählt sie gegen das erste passende (Reihenfolge der Tabelle).
- **Testfixture:** `crawl-shop`: `/produkt/1`…`/produkt/40`, `/blog/1`…`/blog/10`.

---

## G. Rendering

### CRAWL-077 Rendering-Modus wählen
- **Priorität:** P0 (Textmodus) / P1 (Umschalter)
- **Beschreibung:** Drei Modi: **Nur Text** (rohes HTML wird geparst, kein JS; Standard), **JavaScript** (Headless-Chromium rendert, siehe CRAWL-078), **Altes AJAX-Crawling-Schema** (veraltet, siehe CRAWL-087).
- **Eingaben/Konfiguration:** Spider › Rendering › Modus.
- **Ausgabe:** abhängig vom Modus.
- **Akzeptanzkriterien:**
  - [ ] Textmodus startet keinen Browserprozess (Prozessliste während des Crawls).
  - [ ] Wechsel des Modus während eines pausierten Crawls wird verweigert (Hinweis: neuer Crawl nötig) (OS-Festlegung).
  - [ ] Ist JavaScript gewählt und Chromium nicht verfügbar, erscheint vor dem Start eine Fehlermeldung mit Download-/Pfad-Option; kein stiller Rückfall auf Text.
- **Testfixture:** `crawl-js`: `/csr.html` (Inhalt und Links nur per JS), `/ssr.html`.

### CRAWL-078 JavaScript-Rendering mit Headless-Chromium
- **Priorität:** P1
- **Beschreibung:** Jede interne HTML-Seite wird zuerst roh abgerufen und geparst, dann in einem Headless-Chromium mit derselben URL geladen, nach Ablauf des AJAX-Timeouts als DOM serialisiert und erneut geparst. Ressourcen lädt der Browser selbst; die Engine schneidet die Netzwerkanfragen mit.
- **Eingaben/Konfiguration:** Rendering = JavaScript; Anzahl paralleler Browser-Tabs = Threads (CRAWL-112), gedeckelt auf die Anzahl CPU-Kerne (OS-Default).
- **Ausgabe:** Felder aus dem gerenderten DOM (Titel, Meta, H1, Canonical, Directives, Links); zusätzlich Original-Werte für den JavaScript-Tab (anderer Bereich): Filter wie „Nur im gerenderten HTML“, „Durch JS geändert“, „noindex nur im Original-HTML“ basieren auf dem Vergleich beider Fassungen.
- **Akzeptanzkriterien:**
  - [ ] `/csr.html` liefert im JS-Modus Titel und Links aus dem Skript; im Textmodus nicht.
  - [ ] Der Browser verwendet denselben User-Agent, dieselben Header, Cookies, Auth-Daten und Proxy wie der HTTP-Client.
  - [ ] robots.txt gilt auch für Ressourcen, die der Browser anfordert (blockierte werden nicht geladen, CRAWL-083).
  - [ ] Externe Seiten werden nicht gerendert.
  - [ ] Ein abgestürzter Browser-Tab führt zu einem Neustart des Tabs und einer erneuten Verarbeitung der URL (max. 1 Wiederholung), danach Status „Rendering fehlgeschlagen“ mit den Rohdaten.
  - [ ] Service Worker, Benachrichtigungen, Geolokalisierung und Downloads sind im Browser deaktiviert.
  - [ ] Der Browser lädt keine URL zweimal von der Website, wenn die Engine die Antwort bereits hat (Antwort-Wiederverwendung für das Hauptdokument; OS-Festlegung zur Lastreduzierung).
- **Testfixture:** `crawl-js`: `/csr.html`, `/title-changed-by-js.html`, `/noindex-only-raw.html` (noindex im Roh-HTML, per JS entfernt), `/crash.html` (löst `while(true){}` aus).

### CRAWL-079 Link-Herkunft (Roh-HTML vs. gerendertes HTML)
- **Priorität:** P1
- **Beschreibung:** Jeder Link erhält eine Herkunft, damit sichtbar ist, welche Links erst durch JavaScript entstehen. Im JS-Modus werden Links aus beiden Fassungen vereinigt.
- **Eingaben/Konfiguration:** —
- **Ausgabe:** Outlinks-Spalte `Link-Herkunft` ∈ {„HTML“ (nur Roh), „Gerendertes HTML“ (nur gerendert), „HTML & gerendertes HTML“}; Grundlage für Filter „Enthält JavaScript-Links“ (anderer Bereich).
- **Akzeptanzkriterien:**
  - [ ] Link nur im Roh-HTML (per JS entfernt): Herkunft „HTML“ – er wird trotzdem gecrawlt.
  - [ ] Link nur per JS eingefügt: „Gerendertes HTML“.
  - [ ] Link in beiden: „HTML & gerendertes HTML“, nur einmal gezählt.
- **Testfixture:** `crawl-js/link-origins.html` mit allen drei Fällen.

### CRAWL-080 AJAX-Timeout
- **Priorität:** P1
- **Beschreibung:** Zeit, die nach dem Laden der Seite gewartet wird, bevor das DOM übernommen wird.
- **Eingaben/Konfiguration:** Spider › Rendering › „AJAX-Timeout (Sekunden)“ (Standard: 5; Referenz-Default). Bereich 0–60.
- **Akzeptanzkriterien:**
  - [ ] Inhalt, der 2 s nach `load` eingefügt wird, ist bei 5 s vorhanden, bei 1 s nicht.
  - [ ] Die Wartezeit beginnt mit dem `load`-Ereignis; eine Seite, deren `load` nie feuert, wird nach Antwort-Timeout + AJAX-Timeout abgebrochen und mit dem bis dahin vorliegenden DOM verarbeitet.
  - [ ] Wert ist auf Zehntelsekunden einstellbar.
- **Testfixture:** `crawl-js/delayed.html?ms=2000`, `/never-load.html`.

### CRAWL-081 Viewport und Geräte-Voreinstellungen
- **Priorität:** P1
- **Beschreibung:** Fenstergröße des Headless-Browsers. Voreinstellungen für Google-Smartphone und -Desktop werden nach dem Laden auf die Inhaltshöhe vergrößert (bis 8 192 px), damit Lazy-Loading im sichtbaren Bereich ausgelöst wird; andere Geräte bleiben bei ihrem festen Viewport.
- **Eingaben/Konfiguration:** Spider › Rendering › Fenstergröße: Googlebot Mobil (411 × 731, Standard), Googlebot Desktop (1024 × 768), weitere Geräte (z. B. gängige Smartphones/Tablets/Desktop-Auflösungen), Benutzerdefiniert (Breite, Höhe, Skalierungsfaktor, Mobil-Flag, Touch-Flag).
- **Akzeptanzkriterien:**
  - [ ] Im Rendering ist `window.innerWidth` = 411 bei „Googlebot Mobil“ (vor Vergrößerung).
  - [ ] Nach Vergrößerung ist die Fensterhöhe = Dokumenthöhe, max. 8 192 px.
  - [ ] Mobil-Flag setzt `navigator.maxTouchPoints > 0` (bei Touch-Flag) und die Mobil-Emulation im Browser.
  - [ ] Bild mit `loading="lazy"` bei y = 5 000 px wird bei Google-Mobil geladen, bei festem Viewport 375 × 667 ohne Vergrößerung nicht.
- **Testfixture:** `crawl-js/viewport.html` (gibt `innerWidth`/`innerHeight` in den Titel aus), `/lazy-5000.html`.

### CRAWL-082 Gerenderte Screenshots
- **Priorität:** P1
- **Beschreibung:** Beim JS-Rendering wird pro Seite ein Screenshot gespeichert.
- **Eingaben/Konfiguration:** Spider › Rendering › „Gerenderte Seiten-Screenshots aktivieren“ (OS-Default: an, sobald der JS-Modus gewählt wird; im Textmodus nicht wählbar); Screenshot-Breite/-Höhe in px (OS-Default: Viewport aus CRAWL-081), „An Inhalt anpassen“ (Höhe = Dokumenthöhe, max. 8 192 px), Wartezeit nach Größenänderung (ms, OS-Default 300, Bereich 0–10 000), Format PNG (Standard) / JPEG (Qualität 80, OS-Erweiterung).
- **Ausgabe:** Detailbereich „Gerenderte Seite“ mit Bild und Liste blockierter Ressourcen; Sammelexport der Screenshots (je Seite eine Datei, Dateiname aus der URL abgeleitet und dateisystemsicher kodiert).
- **Akzeptanzkriterien:**
  - [ ] Einstellung 1024 × 768 ohne „An Inhalt anpassen“: Bilddatei ist exakt 1024 × 768 px (× Skalierungsfaktor des Viewports, falls ≠ 1).
  - [ ] „An Inhalt anpassen“ bei einer 3 000 px hohen Seite: Bildhöhe 3 000 px (± 1 px); bei einer 20 000 px hohen Seite: 8 192 px.
  - [ ] Inhalt, der 200 ms nach der Größenänderung nachgeladen wird, ist bei Wartezeit 300 ms im Bild sichtbar, bei 0 ms nicht (Pixelprüfung einer farbigen Fläche).
  - [ ] Screenshots werden in der Crawl-Datei gespeichert (komprimiert) und sind nach Schließen und erneutem Öffnen verfügbar (Byte-Vergleich).
  - [ ] Option aus: es wird kein Screenshot erzeugt (Anzahl gespeicherter Bilder = 0), die Detailansicht zeigt „Screenshots deaktiviert“; die mittlere Renderzeit je Seite sinkt messbar (Benchmark: ≥ 10 % schneller bei 100 Seiten).
  - [ ] Sammelexport von 100 Seiten erzeugt 100 Dateien; URLs mit Query-String erzeugen keine Kollision.
  - [ ] Im Textmodus ist die Option ausgegraut und der Detailbereich zeigt „Nur im JavaScript-Rendering-Modus verfügbar“.
- **Testfixture:** `crawl-js/ssr.html`, `/tall-3000.html`, `/tall-20000.html`, `/late-paint.html` (färbt 200 ms nach `resize` eine Fläche rot), `/q.html?a=1` und `/q.html?a=2`.

### CRAWL-083 Blockierte Ressourcen beim Rendern
- **Priorität:** P1
- **Beschreibung:** Ressourcen, die der Browser für das Rendern benötigt, die aber durch robots.txt gesperrt sind oder mit Fehler antworten, werden je Seite erfasst.
- **Eingaben/Konfiguration:** —
- **Ausgabe:** Filter „Antwortcodes › Blockierte Ressource“; je Seite Liste im Detailbereich „Gerenderte Seite“; Filter „Seiten mit blockierten Ressourcen“ im JavaScript-Tab (anderer Bereich).
- **Akzeptanzkriterien:**
  - [ ] JS-Datei unter einem `Disallow`-Pfad wird vom Browser nicht geladen und als blockierte Ressource gemeldet.
  - [ ] Wird robots.txt ignoriert, gibt es keine blockierten Ressourcen durch robots.txt.
  - [ ] Ressource mit 404/500 erscheint ebenfalls als blockiert (Status mitgespeichert).
  - [ ] Eigene robots.txt (CRAWL-102) kann eine Ressource freigeben – Testlauf zeigt danach keine Blockierung.
- **Testfixture:** `crawl-js/blocked.html` lädt `/private-js/app.js` (robots `Disallow: /private-js/`) und `/missing.css` (404).

### CRAWL-084 JavaScript-Fehler / Konsolen-Log
- **Priorität:** P2
- **Beschreibung:** Konsolenmeldungen und unbehandelte Ausnahmen während des Renderns werden je Seite gespeichert.
- **Eingaben/Konfiguration:** Spider › Rendering › „JavaScript-Fehlerberichte aktivieren“ (Standard: aus).
- **Ausgabe:** Detailbereich „Konsolen-Log“ mit Spalten `Ebene` (Fehler/Warnung/Info), `Meldung`, `Quelle`, `Zeile`, `Zeitpunkt (ms)`.
- **Akzeptanzkriterien:**
  - [ ] `throw new Error("x")` erscheint als Ebene „Fehler“ mit Quelle und Zeile.
  - [ ] Pro Seite werden höchstens 500 Meldungen gespeichert (OS-Default), Rest gezählt.
  - [ ] Interne Browser-Meldungen (z. B. zu blockierten Analytics) werden herausgefiltert (Referenz-Fix 23.2).
- **Testfixture:** `crawl-js/errors.html`.

### CRAWL-085 Shadow DOM und iframes abflachen
- **Priorität:** P2
- **Beschreibung:** Inhalte aus Shadow-Roots (offen und deklarativ) und aus gleichartigen iframes werden in das serialisierte DOM übernommen, so wie Google sie beim Indexieren einbezieht.
- **Eingaben/Konfiguration:** Spider › Rendering › „Shadow DOM abflachen“ (Standard: an), „iframes abflachen“ (Standard: an).
- **Akzeptanzkriterien:**
  - [ ] Link in einem offenen Shadow-Root eines Web-Components wird gefunden (Herkunft „Gerendertes HTML“).
  - [ ] Text eines iframes erscheint in der Wortanzahl der Elternseite, wenn abflachen an; sonst nicht.
  - [ ] Per JS in den `<head>` eingefügte iframes werden nicht in den Body übernommen (Referenz-Fix 20.3).
- **Testfixture:** `crawl-js/shadow.html`, `/iframe-parent.html`.

### CRAWL-086 Website-Archiv (Ordner / WARC)
- **Priorität:** P2
- **Beschreibung:** Alle beim Rendern geladenen Dateien werden als Archiv abgelegt – wahlweise als Verzeichnisbaum (unkomprimiert) oder als WARC-Datei.
- **Eingaben/Konfiguration:** Spider › Rendering › „Website-Archiv aktivieren“ (Standard: aus), Format: Ordner / WARC.
- **Ausgabe:** Sammelexport „Web › Archivierte Website“.
- **Akzeptanzkriterien:**
  - [ ] WARC-Datei ist mit gängigen Werkzeugen lesbar (Validierung mit einem WARC-Parser im Test) und enthält Request- und Response-Records je Ressource.
  - [ ] Ordnerformat bildet Host/Pfad ab; Query-Strings werden in Dateinamen sicher kodiert.
- **Testfixture:** `crawl-site-100` (JS-Modus).

### CRAWL-087 Altes AJAX-Crawling-Schema
- **Priorität:** P2
- **Beschreibung:** Unterstützt das veraltete Schema, bei dem `#!`-URLs bzw. Seiten mit `<meta name="fragment" content="!">` über `?_escaped_fragment_=` abgerufen werden. Gibt es keine Escaped-Fragment-Fassung, wird das rohe HTML verwendet.
- **Eingaben/Konfiguration:** Rendering-Modus „Altes AJAX-Crawling-Schema“.
- **Akzeptanzkriterien:**
  - [ ] Link `/#!seite=2` wird als `/?_escaped_fragment_=seite=2` angefragt, aber unter der `#!`-URL gemeldet.
  - [ ] Seite mit Meta-Fragment wird zusätzlich als `?_escaped_fragment_=` angefragt.
- **Testfixture:** `crawl-ajax-scheme`.

---

## H. Erweitert

### CRAWL-088 Cookie-Verhalten
- **Priorität:** P0
- **Beschreibung:** Steuert, ob und wie Cookies zwischen Requests weitergegeben werden.
- **Eingaben/Konfiguration:** Erweitert › „Cookies“: **Nur Sitzung** (Standard; Cookies gelten nur für den einzelnen Seitenabruf inkl. dessen Weiterleitungen und – bei Rendering – dessen Ressourcen, danach verworfen), **Dauerhaft** (ein gemeinsamer Cookie-Speicher für alle Threads über den ganzen Crawl), **Nicht speichern** (Cookies werden nie zurückgesendet).
- **Ausgabe:** —
- **Akzeptanzkriterien:**
  - [ ] Nur Sitzung: `/set` setzt Cookie und leitet auf `/check` weiter → `/check` erhält das Cookie; ein späterer, separater Abruf von `/check2` erhält es nicht.
  - [ ] Dauerhaft: `/check2` erhält das Cookie, auch aus einem anderen Thread.
  - [ ] Nicht speichern: `/check` erhält kein Cookie.
  - [ ] Zu Beginn jedes neuen Crawls ist der Cookie-Speicher leer; Cookies aus Formular-Login (CRAWL-120) werden danach eingespielt.
  - [ ] Domain-/Pfad-/Secure-/Ablauf-Regeln nach RFC 6265 werden eingehalten (Secure-Cookie nicht über http).
  - [ ] Dauerhafte Cookies werden nicht in der Crawl-Datei gespeichert (nur auf Wunsch über CRAWL-062 als Daten).
- **Testfixture:** `crawl-cookies`: `/set` (Set-Cookie + 302 → `/check`), `/check`, `/check2` (Body zeigt empfangene Cookies), `/secure-only`.

### CRAWL-089 Nicht indexierbare URLs bei Problemen ignorieren
- **Priorität:** P1
- **Beschreibung:** Inhaltliche Problemfilter (z. B. fehlender Titel, doppelte Beschreibung) betrachten nur indexierbare Seiten. Die Option kann abgeschaltet werden, z. B. für Staging-Sites mit sitewidem `noindex`.
- **Eingaben/Konfiguration:** Erweitert › „Nicht indexierbare URLs bei On-Page-Filtern ignorieren“ (Standard: an; OS-Default, siehe Q16). Achtung: `02-tabs-onpage.md` (0.3, `issues.ignore_non_indexable`, Annahme A-51) setzt denselben Schlüssel auf `false` – Widerspruch zwischen den Bereichen, Entscheidung durch Orchestrator (Q36); bis dahin gilt für Tests der Wert aus 0.3.
- **Akzeptanzkriterien:**
  - [ ] An: Seite mit `noindex` und fehlendem Titel erscheint nicht im Filter „Titel fehlt“.
  - [ ] Aus: Sie erscheint.
  - [ ] Umschalten nach dem Crawl wirkt sofort auf die Filter (keine Neucrawl nötig; Neuberechnung der Filter).
- **Testfixture:** `crawl-indexability`: `/noindex-notitle`, `/canonicalised-dupe`, `/ok`.

### CRAWL-090 Paginierte URLs bei Duplikat-Filtern ignorieren
- **Priorität:** P2
- **Beschreibung:** Seiten einer Paginierungsserie (mit `rel=prev`) werden bei Duplikatfiltern von Titel/Beschreibung nicht gemeldet.
- **Eingaben/Konfiguration:** Erweitert › „Paginierte URLs bei Duplikat-Filtern ignorieren“ (Standard: an; OS-Default).
- **Akzeptanzkriterien:**
  - [ ] An: `/seite/2` und `/seite/3` mit identischem Titel wie `/seite/1` erscheinen nicht in „Doppelter Titel“.
  - [ ] Aus: Sie erscheinen.
- **Testfixture:** `crawl-pagination`.

### CRAWL-091 noindex respektieren
- **Priorität:** P1
- **Beschreibung:** Seiten mit `noindex` werden weiterhin abgerufen und ihre Links verfolgt, aber aus den Ergebnistabellen ausgeblendet – zur Sicht „wie ein Suchmaschinen-Index“.
- **Eingaben/Konfiguration:** Erweitert › „noindex respektieren“ (Standard: aus).
- **Akzeptanzkriterien:**
  - [ ] An: `/noindex` fehlt in allen Tabs; `/nur-von-noindex` (nur dort verlinkt) wird gecrawlt und angezeigt.
  - [ ] Die Ausblendung gilt auch für Exporte und Berichte.
  - [ ] Request-Log enthält `/noindex` (wurde abgerufen).
- **Testfixture:** `crawl-indexability`: `/noindex` (Meta), `/noindex-header` (X-Robots-Tag), beide verlinken exklusiv `/nur-von-noindex`.

### CRAWL-092 Canonical respektieren
- **Priorität:** P1
- **Beschreibung:** Kanonisierte Seiten (Canonical zeigt auf andere URL) werden abgerufen und ihre Links verfolgt, aber ausgeblendet.
- **Eingaben/Konfiguration:** Erweitert › „Canonical respektieren“ (Standard: aus).
- **Akzeptanzkriterien:**
  - [ ] An: `/variante?farbe=rot` (canonical → `/variante`) fehlt in den Tabellen; `/variante` wird angezeigt.
  - [ ] Selbstreferenzierende Canonicals führen nicht zur Ausblendung.
- **Testfixture:** `crawl-canonicals`: `/variante`, `/variante?farbe=rot`.

### CRAWL-093 next/prev respektieren
- **Priorität:** P2
- **Beschreibung:** Nur die erste Seite einer Paginierungsserie (Seite mit `rel=next`, ohne `rel=prev`) wird angezeigt; Folgeseiten werden gecrawlt, aber ausgeblendet.
- **Eingaben/Konfiguration:** Erweitert › „next/prev respektieren“ (Standard: aus).
- **Akzeptanzkriterien:**
  - [ ] An: `/seite/1` sichtbar; `/seite/2`…`/seite/5` ausgeblendet, aber im Request-Log.
- **Testfixture:** `crawl-pagination`.

### CRAWL-094 Selbstreferenzierenden Meta-Refresh respektieren
- **Priorität:** P2
- **Beschreibung:** Eine Seite, die sich per Meta-Refresh auf sich selbst neu lädt, gilt standardmäßig als nicht indexierbar (Indexierbarkeitsstatus „Meta Refresh auf sich selbst“, wird im Indexierbarkeitsbereich ausgewertet). Die Option schaltet diese Einstufung ab.
- **Eingaben/Konfiguration:** Erweitert › „Selbstreferenzierenden Meta-Refresh respektieren“ (Standard: an).
- **Akzeptanzkriterien:**
  - [ ] An: `/meta-self-url` (Refresh mit URL auf sich selbst) ist nicht indexierbar.
  - [ ] Aus: indexierbar.
  - [ ] Refresh ohne URL (`content="30"`) wird wie selbstreferenzierend behandelt (OS-Festlegung, siehe Q).
- **Testfixture:** `crawl-refresh`: `/meta-self-url`, `/meta-self`.

### CRAWL-095 Antwort-Timeout
- **Priorität:** P0
- **Beschreibung:** Höchstdauer, die auf eine Antwort gewartet wird.
- **Eingaben/Konfiguration:** Erweitert › „Antwort-Timeout (Sekunden)“ (Standard: 20; Referenz-Default). Zusätzlich intern: Verbindungsaufbau-Timeout = min(10 s, Antwort-Timeout) (OS-Default).
- **Ausgabe:** Status „Antwort-Timeout“ bzw. „Verbindungs-Timeout“ (CRAWL-013).
- **Akzeptanzkriterien:**
  - [ ] Timeout 2 s, Server antwortet nach 3 s: Status „Antwort-Timeout“ nach 2–2,5 s.
  - [ ] Server antwortet nach 1 s: normaler 200.
  - [ ] Ein langsam tröpfelnder Body (1 Byte/s) wird nach Ablauf des Timeouts abgebrochen (Timeout gilt für die gesamte Antwort, nicht nur das erste Byte; OS-Festlegung).
- **Testfixture:** `crawl-timing`: `/slow?ms=3000`, `/slow?ms=1000`, `/trickle`.

### CRAWL-096 Wiederholung bei 5xx-Antworten
- **Priorität:** P1
- **Beschreibung:** URLs mit 5xx-Antwort werden automatisch erneut angefragt.
- **Eingaben/Konfiguration:** Erweitert › „5xx-Wiederholungen“ (Anzahl; OS-Default: 0, siehe Q); Pause zwischen Versuchen: exponentiell ab 1 s (1 s, 2 s, 4 s …; OS-Default).
- **Ausgabe:** gespeichert wird die letzte Antwort; Spalte `Versuche` `attempts` (OS-Erweiterung).
- **Akzeptanzkriterien:**
  - [ ] Wiederholungen 2, Server liefert 503, 503, 200: Ergebnis 200, `Versuche` = 3.
  - [ ] Wiederholungen 2, Server liefert dreimal 500: Ergebnis 500, `Versuche` = 3.
  - [ ] Wiederholungen 0: genau ein Request.
  - [ ] `Retry-After` (Sekunden oder HTTP-Datum) bei 503 wird beachtet, gedeckelt auf 60 s.
  - [ ] Wiederholungen blockieren keine anderen Threads (Wartezeit nicht im Worker).
- **Testfixture:** `crawl-retries`: `/flaky?fail_first=2&code=503`, `/always500`, `/retry-after`.

### CRAWL-097 Wiederholung bei Netzwerkfehlern und 429
- **Priorität:** P1
- **Beschreibung:** OpenSpider-Erweiterung zur Robustheit: Verbindungsfehler und Timeouts sowie 429 (Too Many Requests) können ebenfalls wiederholt werden; bei 429 wird zusätzlich das Tempo gedrosselt.
- **Eingaben/Konfiguration:** Erweitert › „Wiederholungen bei Verbindungsfehlern“ (OS-Default: 1), „Bei 429 automatisch drosseln“ (OS-Default: an).
- **Akzeptanzkriterien:**
  - [ ] Verbindung, die beim ersten Versuch zurückgesetzt wird und beim zweiten klappt, ergibt 200.
  - [ ] Nach einem 429 halbiert sich die effektive Rate für diesen Host für 60 s (sichtbar in der Statusleiste: „Gedrosselt“), `Retry-After` wird beachtet.
  - [ ] DNS-Fehler werden nicht wiederholt.
- **Testfixture:** `crawl-retries`: `/reset-once`, `/rate-limited` (429 bei mehr als 2 Anfragen/s).

---
## I. robots.txt

### CRAWL-098 robots.txt abrufen und auswerten
- **Priorität:** P0
- **Beschreibung:** Vor der ersten Anfrage an einen Host (Schema + Host + Port) wird dessen `/robots.txt` abgerufen und nach RFC 9309 sowie dem dokumentierten Google-Verhalten ausgewertet. Jede URL wird vor dem Einreihen gegen die Regeln des eingestellten robots-User-Agents geprüft.
- **Eingaben/Konfiguration:** robots.txt-Modus (CRAWL-099); robots-User-Agent-Token (CRAWL-114).
- **Ausgabe:** blockierte URLs mit Statuscode 0, Status „Durch robots.txt blockiert“; Spalten `Gesperrt durch Zeile` (Zeilennummer und Regeltext) und `robots.txt-URL` (OS-Erweiterung, auch im Tester CRAWL-102).
- **Akzeptanzkriterien:**
  - [ ] Gruppenauswahl: Die spezifischste passende `User-agent`-Gruppe gewinnt (Token-Vergleich case-insensitiv); mehrere Gruppen für denselben Agent werden zusammengeführt; `*` nur, wenn keine spezifische passt.
  - [ ] Regelvorrang: längste passende Regel (nach Zeichenzahl des Musters) gewinnt; bei Gleichstand gewinnt `Allow`.
  - [ ] Platzhalter `*` und Ende-Anker `$` werden unterstützt; Vergleich auf dem prozentkodierten Pfad inklusive Query.
  - [ ] `Disallow:` (leer) erlaubt alles.
  - [ ] Unbekannte Direktiven werden ignoriert; `Crawl-delay` wird gelesen und als Hinweis angezeigt, aber nicht angewandt (OS-Festlegung; Tempo steuert CRAWL-113).
  - [ ] BOM, CRLF/LF/CR-Zeilenenden, Kommentare (`#`) und Leerzeichen vor dem Doppelpunkt werden toleriert.
  - [ ] Nur die ersten 500 KiB werden ausgewertet (RFC 9309).
  - [ ] Weiterleitungen beim robots.txt-Abruf werden bis zu 5 Hops verfolgt.
  - [ ] Ein Host-Wechsel (anderer Port, http vs. https) bedeutet eine eigene robots.txt.
  - [ ] Jeder Content-Type mit Textinhalt wird als robots.txt gelesen (Referenz-Fix 18.5).
  - [ ] Sehr große robots.txt (mehrere MB) führen zu keinem Absturz (Referenz-Fix 19.7).
  - [ ] Die robots.txt wird je Host genau einmal pro Crawl abgerufen und erscheint selbst als URL in den Tabellen (Inhaltstyp Text).
- **Testfixture:** `crawl-robots`: `robots.txt` mit Gruppen für `*`, `OpenSpider`, `Googlebot`; Regeln `Disallow: /privat/`, `Allow: /privat/oeffentlich`, `Disallow: /*.pdf$`, `Disallow: /*?sessionid=`, `Allow: /seite$`, `Disallow: /seite`; Links auf alle Grenzfälle; Varianten-Hosts `bom.site.test` (BOM + CRLF), `big.site.test` (3 MB), `redirect.site.test` (robots → 301 → andere Datei).

### CRAWL-099 robots.txt-Modi
- **Priorität:** P0
- **Beschreibung:** Drei Modi: **Respektieren** (Standard im Spider-Modus), **Ignorieren** (Datei wird nicht abgerufen, keine Regeln), **Ignorieren, aber melden** (Datei wird abgerufen, URLs werden trotzdem gecrawlt, aber als „wäre blockiert“ gekennzeichnet).
- **Eingaben/Konfiguration:** Konfiguration › robots.txt › Einstellungen.
- **Ausgabe:** im Modus „Ignorieren, aber melden“ Spalte `Durch robots.txt blockiert` = ja und `Gesperrt durch Zeile` gefüllt, Statuscode = echter Code.
- **Akzeptanzkriterien:**
  - [ ] Respektieren: `/privat/x` nicht im Request-Log; Zeile mit Status „Durch robots.txt blockiert“.
  - [ ] Ignorieren: kein Abruf von `/robots.txt` im Request-Log; `/privat/x` mit 200 gecrawlt.
  - [ ] Ignorieren, aber melden: `/robots.txt` und `/privat/x` im Request-Log; `/privat/x` hat Statuscode 200 und Markierung „wäre blockiert“.
  - [ ] Im Listenmodus ist „Ignorieren“ voreingestellt (CRAWL-028).
- **Testfixture:** `crawl-robots`.

### CRAWL-100 Fehlerfälle beim robots.txt-Abruf
- **Priorität:** P0
- **Beschreibung:** Verhalten, wenn die robots.txt nicht regulär abrufbar ist. **Die Referenz-Doku sagt hierzu nichts.** Das gesamte Verhalten dieses Eintrags ist eine **OS-Festlegung**, angelehnt an RFC 9309 und Googles veröffentlichte Praxis; es ist per Black-Box-Test gegen die Referenz zu prüfen (Q30). Risiko: Die Referenz crawlt bei 5xx womöglich einfach weiter; dann wäre OpenSpider strenger. Damit Nutzer das Referenzverhalten nachstellen können, ist der Umgang mit Serverfehlern konfigurierbar.
- **Eingaben/Konfiguration:** robots.txt › „Bei nicht erreichbarer robots.txt“ (OS-Erweiterung, Schlüssel `robots.unreachable_policy`): **Host als gesperrt behandeln** (OS-Default) / **Alles erlauben**. Wiederholungen: 2 (OS-Default), Abstand 5 s.
- **Ausgabe:** Hinweis in der Crawl-Übersicht je Host mit Abrufstatus, z. B. „robots.txt: 503 – Host gesperrt“ bzw. „robots.txt: 404 – keine Regeln“; Spalte `robots.txt-Status` (OS-Erweiterung) in der Host-Übersicht.
- **Akzeptanzkriterien:**
  - [ ] 2xx mit Regeln: Regeln gelten (CRAWL-098).
  - [ ] 404, 410, 401, 403 und alle übrigen 4xx außer 429: alles erlaubt; Request-Log zeigt `/seite` angefragt; Hinweis „keine Regeln“.
  - [ ] 500, 503, 429, Timeout, Verbindungsabbruch, DNS-Fehler bei Policy „Host als gesperrt behandeln“: Request-Log zeigt genau 3 Abrufe von `/robots.txt` (1 + 2 Wiederholungen, Abstand ≥ 5 s) und **keinen** Abruf von `/seite`; `/seite` erscheint mit Statuscode 0 und Status „Durch robots.txt blockiert (robots.txt nicht erreichbar)“ im Filter „Durch robots.txt blockiert“.
  - [ ] Liefert die robots.txt beim 2. Versuch 200, gelten deren Regeln und `/seite` wird normal gecrawlt.
  - [ ] Dieselben Fehlerfälle bei Policy „Alles erlauben“: `/seite` wird angefragt; Hinweis „robots.txt nicht erreichbar – ohne Regeln gecrawlt“.
  - [ ] Weiterleitung der robots.txt auf eine Fehlerantwort wird wie diese Fehlerantwort behandelt (`→ 404` = alles erlaubt, `→ 503` = Policy greift); mehr als 5 Weiterleitungen = wie 404.
  - [ ] Leere Datei (200, 0 Bytes): alles erlaubt.
  - [ ] HTML statt robots.txt (200, `text/html`, Soft-404-Seite): wird zeilenweise geparst, enthält keine gültigen Regeln, also alles erlaubt.
  - [ ] Im robots.txt-Modus „Ignorieren“ (CRAWL-099) werden keine dieser Abrufe ausgeführt.
- **Testfixture:** `crawl-robots-errors`: Hosts `r404.site.test`, `r403.site.test`, `r500.site.test`, `r503-once.site.test` (`fail_first_n=1`, dann 200 mit `Disallow: /seite`), `r429.site.test`, `rtimeout.site.test` (`hang`), `rempty.site.test`, `rhtml.site.test`, `rredir404.site.test`, `rredir503.site.test`, je mit `/seite`.

### CRAWL-101 Anzeige blockierter URLs (intern/extern)
- **Priorität:** P1
- **Beschreibung:** Steuert, ob durch robots.txt blockierte URLs in den Tabellen erscheinen.
- **Eingaben/Konfiguration:** robots.txt › „Interne durch robots.txt blockierte URLs anzeigen“ (Standard: an), „Externe durch robots.txt blockierte URLs anzeigen“ (Standard: aus). Beide nur im Modus „Respektieren“ aktiv.
- **Akzeptanzkriterien:**
  - [ ] Standard: intern blockierte URLs erscheinen im Tab „Intern“ mit Statuscode 0; extern blockierte erscheinen nicht (die externe robots.txt wird dafür trotzdem abgerufen, damit der Status bestimmt werden kann – nur wenn externe Links gecrawlt werden).
  - [ ] Externe Option an: extern blockierte URLs erscheinen im Tab „Extern“ mit Statuscode 0.
  - [ ] Interne Option aus: blockierte interne URLs erscheinen nicht; Outlinks auf sie bleiben sichtbar.
  - [ ] Externe Option aus: Die externe URL wird nicht angefragt, wenn ihre robots.txt sie sperrt (Respekt gegenüber Dritten).
- **Testfixture:** `crawl-robots` + `other.test/robots.txt` mit `Disallow: /`.

### CRAWL-102 Eigene robots.txt pro Subdomain und Tester
- **Priorität:** P1
- **Beschreibung:** Für jede Subdomain kann eine eigene robots.txt hinterlegt werden, die statt der Live-Datei gilt – nur für diesen Crawl, ohne die Website zu ändern. Ein Tester prüft einzelne URLs.
- **Eingaben/Konfiguration:** robots.txt › „Eigene robots.txt“: Liste von Subdomains (Hinzufügen/Entfernen), je Subdomain ein Editor (vorbefüllbar mit der Live-Datei über „Live-Datei laden“); Tester-Feld (URL) mit Ergebnis.
- **Ausgabe:** Tester zeigt „Erlaubt“/„Blockiert“, die entscheidende Zeile hervorgehoben und den verwendeten User-Agent.
- **Akzeptanzkriterien:**
  - [ ] Eigene Datei mit `Disallow: /blog/` sperrt `/blog/` beim Crawl, obwohl die Live-Datei es erlaubt.
  - [ ] Subdomains ohne eigene Datei verwenden weiterhin die Live-Datei.
  - [ ] Tester verwendet den in der Konfiguration eingestellten robots-User-Agent; Änderung des UA ändert das Testergebnis sofort.
  - [ ] Eigene Dateien werden im Konfigurationsprofil gespeichert.
  - [ ] Die eigene Datei überschreibt auch das Verhalten aus CRAWL-100 (Live-Datei liefert 500 → eigene Datei gilt).
- **Testfixture:** `crawl-robots`, `crawl-robots-errors/r500`.

---

## J. URL-Umschreibung

Gemeinsame Regeln: Umschreibungen gelten ausschließlich für URLs, die **während des Crawls entdeckt** werden (aus Links, Weiterleitungs-Zielen, Canonicals, Hreflang, Sitemaps). Sie gelten **nicht** für die Start-URL im Spider-Modus und **nicht** für die im Listenmodus hochgeladenen URLs (Referenzverhalten). Links, die aus Listen-URLs heraus entdeckt werden (Listenmodus mit Tiefe ≥ 1), werden dagegen umgeschrieben. Reihenfolge: (1) Parameter entfernen, (2) Regex-Ersetzung, (3) Kleinschreibung, (4) Prozent-Kodierung, danach Normalisierung (CRAWL-003). Die ursprüngliche URL bleibt im Outlink-Datensatz erhalten (Spalte `Original-URL`, OS-Erweiterung).

### CRAWL-103 Parameter entfernen
- **Priorität:** P1
- **Beschreibung:** Liste von Parameternamen, die aus jeder gefundenen URL entfernt werden (z. B. Sitzungs-IDs, Tracking-Parameter).
- **Eingaben/Konfiguration:** URL-Umschreibung › „Parameter entfernen“: eine Zeile pro Parametername (exakte Übereinstimmung, case-sensitiv); Schalter „Alle Parameter entfernen“ (OS-Komfortoption, entspricht Entfernen des gesamten Query-Strings).
- **Akzeptanzkriterien:**
  - [ ] `sid` entfernt: `/a?sid=1&x=2` → `/a?x=2`; `/a?sid=1` → `/a` (kein `?` übrig).
  - [ ] Mehrfach vorkommender Parameter wird überall entfernt.
  - [ ] `utm_source` entfernt nicht `utm_source2`.
  - [ ] Reihenfolge der übrigen Parameter bleibt erhalten.
  - [ ] Fragment bleibt unberührt (wird separat behandelt).
- **Testfixture:** `crawl-rewrite`: Seite mit Links `/a?sid=1&x=2`, `/a?x=2&sid=9`, `/b?sid=1`, `/c?utm_source=x&utm_source2=y`.

### CRAWL-104 Regex-Ersetzung
- **Priorität:** P1
- **Beschreibung:** Liste von Regeln (Regex → Ersetzung), die nacheinander auf jede gefundene URL angewendet werden. Typische Fälle: http→https umschreiben, `www` ergänzen/entfernen, Staging-Links auf Live mappen, Query vollständig entfernen.
- **Eingaben/Konfiguration:** URL-Umschreibung › „Regex ersetzen“: Tabelle (Regex, Ersetzung), Reihenfolge änderbar.
- **Akzeptanzkriterien:**
  - [ ] `http://` → `https://` macht aus jedem http-Link einen https-Link.
  - [ ] `\?.*` → `` (leer) entfernt jeden Query-String.
  - [ ] Rückreferenzen `$1` funktionieren: `(.*)/index\.html$` → `$1/`.
  - [ ] Jede Regel ersetzt alle Vorkommen (global), nicht nur das erste.
  - [ ] Ist das Ergebnis keine gültige URL, bleibt die Original-URL erhalten und eine Warnung wird protokolliert.
  - [ ] Die Start-URL wird nicht umgeschrieben: Start `http://site.test/` mit Regel `http://` → `https://` erzeugt im Request-Log einen Abruf von `http://site.test/`.
  - [ ] Listenmodus: Eine hochgeladene URL `http://site.test/x/index.html` wird unverändert angefragt; bei Tiefe 1 werden die von ihr verlinkten URLs umgeschrieben.
  - [ ] Ungültige Regex wird beim Speichern abgewiesen.
- **Testfixture:** `crawl-rewrite`: Links mit `http://site.test/…`, `/x/index.html`, `/y?foo=bar`; Liste `rewrite-list.txt` mit `http://site.test/x/index.html`.

### CRAWL-105 Gefundene URLs kleinschreiben
- **Priorität:** P2
- **Beschreibung:** Konvertiert Pfad und Query aller gefundenen URLs in Kleinbuchstaben – für Server, die Groß-/Kleinschreibung ignorieren und so Duplikate erzeugen.
- **Eingaben/Konfiguration:** URL-Umschreibung › „Alle gefundenen URLs kleinschreiben“ (Standard: aus).
- **Akzeptanzkriterien:**
  - [ ] `/Produkt/ABC?Farbe=Rot` → `/produkt/abc?farbe=rot`; `/produkt/abc` wird danach nur einmal gecrawlt.
  - [ ] Prozent-Escapes bleiben in Großbuchstaben (`%C3%A9` bleibt, sofern Kodierungsmodus Großbuchstaben).
- **Testfixture:** `crawl-rewrite/case.html`.

### CRAWL-106 Prozent-Kodierungsmodus
- **Priorität:** P2
- **Beschreibung:** Legt fest, ob Hex-Ziffern in Prozent-Escapes groß (Standard, `%C3%A9`) oder klein (`%c3%a9`) geschrieben werden – für Server, die nur eine Schreibweise akzeptieren und sonst weiterleiten oder Fehler liefern.
- **Eingaben/Konfiguration:** URL-Umschreibung › „Prozent-Kodierung“: Großbuchstaben (Standard) / Kleinbuchstaben / Unverändert (so wie im Quelltext).
- **Akzeptanzkriterien:**
  - [ ] Großbuchstaben: `/caf%c3%a9` wird als `/caf%C3%A9` angefragt.
  - [ ] Kleinbuchstaben: umgekehrt.
  - [ ] Unverändert: genau die Schreibweise des Quelltexts wird angefragt.
  - [ ] Alte Profile ohne diese Einstellung laden mit „Großbuchstaben“ (Referenz-Fix 23.1).
- **Testfixture:** `crawl-rewrite/encoding.html`; Server antwortet auf `%c3%a9` mit 301 auf `%C3%A9`.

### CRAWL-107 Testfeld für Umschreibungen
- **Priorität:** P1
- **Beschreibung:** Im Umschreibungsdialog kann eine Beispiel-URL eingegeben werden; das Ergebnis aller aktiven Umschreibungen wird sofort angezeigt.
- **Eingaben/Konfiguration:** Feld „Test-URL“.
- **Ausgabe:** „Umgeschriebene URL“ und Zwischenergebnisse je Schritt.
- **Akzeptanzkriterien:**
  - [ ] Ergebnis aktualisiert sich bei jeder Änderung einer Regel ohne Speichern.
  - [ ] Das Testfeld nutzt exakt dieselbe Funktion wie die Engine (gemeinsamer Code in `os-url`; Unit-Test vergleicht beide).
- **Testfixture:** Unit-Tests in `os-url` mit den Fällen aus CRAWL-103–106.

---

## K. Bereichssteuerung

### CRAWL-108 CDNs als intern behandeln
- **Priorität:** P1
- **Beschreibung:** Liste von Domains oder Domain+Pfad-Präfixen, die wie interne URLs behandelt werden (in Tab „Intern“, Links werden verfolgt), z. B. ein CDN für Bilder oder ein Unterordner einer anderen Domain.
- **Eingaben/Konfiguration:** Konfiguration › CDNs: eine Zeile pro Eintrag (`cdn.other.test` oder `other.test/assets/`).
- **Akzeptanzkriterien:**
  - [ ] Eintrag `cdn.other.test`: Bilder von dort erscheinen unter „Intern › Bilder“.
  - [ ] Eintrag `other.test/assets/`: `other.test/assets/a.css` intern, `other.test/blog/` extern.
  - [ ] HTML-Seiten auf CDN-Hosts werden wie interne Seiten geparst und ihre Links verfolgt.
  - [ ] Vergleich ist case-insensitiv für Host, case-sensitiv für Pfad; Schema wird ignoriert.
- **Testfixture:** `crawl-cdn`: `site.test` bindet `cdn.other.test/img.png`, `other.test/assets/a.css`, verlinkt `other.test/blog/` und `cdn.other.test/seite.html` (mit Link auf `cdn.other.test/seite2.html`).

### CRAWL-109 Include-Regeln (Regex)
- **Priorität:** P0
- **Beschreibung:** Liste von Regex-Mustern. Ist sie nicht leer, werden nur URLs gecrawlt, die auf mindestens ein Muster passen. Die Start-URL wird immer gecrawlt; damit ein Include-Crawl in Gang kommt, muss sie auf passende URLs verlinken.
- **Eingaben/Konfiguration:** Konfiguration › Include: eine Regex pro Zeile.
- **Abgleichsemantik:** Die Referenz-Doku verlangt für Include ausdrücklich, dass das Muster die **gesamte** URL abdeckt (Teilstücke müssen mit `.*` eingerahmt werden). OpenSpider übernimmt das als **Vollübereinstimmung** und wendet dieselbe Semantik auf Exclude an (CRAWL-110), damit beide Listen gleich gelesen werden. Die Exclude-Beispiele der Referenz-Doku lassen sich ebenfalls als Vollübereinstimmung lesen, sind in verkürzten Wiedergaben aber mehrdeutig – Q25 hält die Black-Box-Bestätigung für beide Listen fest. Ein Schalter „Teilübereinstimmung genügt“ (OS-Erweiterung, Schlüssel `scope.regex_partial_match`, Standard: aus) erlaubt die Suchsemantik für Nutzer, die sie bevorzugen; er gilt für Include und Exclude gemeinsam.
- **Akzeptanzkriterien:**
  - [ ] Abgeglichen wird gegen die vollständige, prozentkodierte, umgeschriebene URL (inkl. Schema, Host, Query), so wie sie in `URL-kodierte Adresse` (CRAWL-003) steht.
  - [ ] Vollübereinstimmung: Muster `.*/blog/.*` passt auf `https://site.test/blog/a`; Muster `/blog/` passt auf **keine** URL; Muster `https://site\.test/blog/a` passt nur auf genau diese URL, nicht auf `https://site.test/blog/a?x=1`.
  - [ ] Mit `scope.regex_partial_match = true` passt `/blog/` auf `https://site.test/blog/a` und `https://site.test/blog/a?x=1`.
  - [ ] Muster `.*/blog/.*`: `/blog/a`, `/blog/b` gecrawlt, `/shop/x` nicht; `/shop/x` erscheint aber als Outlink der Startseite (ohne Statuscode).
  - [ ] Start-URL `/` passt nicht auf `.*/blog/.*`, wird aber gecrawlt.
  - [ ] Include wirkt auch auf Ressourcen: `/img/a.png` wird bei Muster `.*/blog/.*` nicht angefragt.
  - [ ] Im Listenmodus werden nicht passende Listen-URLs vor dem Start entfernt und im Upload-Dialog unter `Durch Include entfernt` gezählt.
  - [ ] Leere Zeilen werden ignoriert; Groß-/Kleinschreibung wird beachtet (`/Blog/c` passt nicht auf `.*/blog/.*`), `(?i).*/blog/.*` passt auch auf `/Blog/c`.
  - [ ] Ein Muster, das nach dem Crawl auf keine URL gepasst hat, wird in der Crawl-Übersicht als „Include-Zeile n ohne Treffer“ gemeldet (Hilfe bei vergessenem `.*`).
- **Testfixture:** `crawl-include-exclude`: `/` verlinkt `/blog/a`, `/blog/a?x=1`, `/blog/b`, `/shop/x`, `/Blog/c`; `/blog/a` bindet `/img/a.png`.

### CRAWL-110 Exclude-Regeln (Regex)
- **Priorität:** P0
- **Beschreibung:** Liste von Regex-Mustern. URLs, die auf ein Muster passen, werden nicht angefragt und erscheinen nicht in den Tabellen (wohl aber als Outlinks). Exclude hat Vorrang vor Include. Die Start-URL ist ausgenommen.
- **Eingaben/Konfiguration:** Konfiguration › Exclude: eine Regex pro Zeile. Abgleichsemantik wie CRAWL-109 (Vollübereinstimmung, gemeinsamer Schalter für Teilübereinstimmung).
- **Akzeptanzkriterien:**
  - [ ] `.*\.pdf` schließt `https://site.test/handbuch.pdf` aus, aber nicht `https://site.test/handbuch.pdf?v=2` (Vollübereinstimmung endet am Ende der URL).
  - [ ] `.*[?&]sortierung=.*` schließt `/liste?sortierung=preis` und `/liste?seite=2&sortierung=preis` aus, nicht `/liste?seite=2`.
  - [ ] `.*/artikel/[0-9]{4}/.*` schließt `/artikel/2024/titel` aus, nicht `/artikel/24/titel`.
  - [ ] `https://site\.test/kalender/.*` schließt den gesamten Kalender-Ordner aus, aber nicht `https://blog.site.test/kalender/x`.
  - [ ] Abgleich gegen die kodierte URL: Ein Link auf `/a b` (Leerzeichen) wird durch `.*%20.*` ausgeschlossen, nicht durch `.* .*`; ein Link auf `/über` durch `.*%C3%BC.*`.
  - [ ] Ohne `.*` wirkt ein Teilmuster nicht: `\.pdf` schließt nichts aus (außer `scope.regex_partial_match = true`).
  - [ ] URL, die sowohl Include als auch Exclude erfüllt, wird nicht gecrawlt.
  - [ ] Ein Weiterleitungsziel, das ausgeschlossen ist, wird nicht angefragt (CRAWL-014).
  - [ ] Exclude-Treffer werden pro Crawl gezählt (Crawl-Übersicht „Durch Exclude übersprungen“).
  - [ ] Exclude kann während einer Pause geändert werden und wirkt auf alle noch nicht abgerufenen URLs der Warteschlange (sie werden entfernt).
- **Testfixture:** `crawl-include-exclude`: zusätzlich `/handbuch.pdf`, `/handbuch.pdf?v=2`, `/liste?sortierung=preis`, `/liste?seite=2&sortierung=preis`, `/liste?seite=2`, `/artikel/2024/titel`, `/artikel/24/titel`, `/kalender/2025-01`, `blog.site.test/kalender/x`, `/a b`, `/über`.

### CRAWL-111 Regex-Tester für Include/Exclude
- **Priorität:** P1
- **Beschreibung:** In den Include-/Exclude-Dialogen kann eine Test-URL eingegeben werden; angezeigt wird, ob und durch welche Zeile sie ein-/ausgeschlossen wird.
- **Eingaben/Konfiguration:** Feld „Test-URL“.
- **Ausgabe:** „Eingeschlossen“/„Ausgeschlossen“/„Nicht eingeschlossen“ und Nummer der passenden Zeile.
- **Akzeptanzkriterien:**
  - [ ] Der Tester kodiert die Eingabe wie die Engine, bevor er abgleicht (Eingabe `https://site.test/a b` passt auf `.*%20.*`).
  - [ ] Der Tester verwendet dieselbe Abgleichsemantik wie die Engine (Vollübereinstimmung bzw. Teilübereinstimmung laut Schalter) und zeigt bei einem Muster, das nur als Teilstück passen würde, den Hinweis „passt nur teilweise – `.*` ergänzen?“.
  - [ ] Syntaxfehler in einer Zeile werden zeilengenau markiert; die übrigen Zeilen werden trotzdem getestet.
- **Testfixture:** Unit-Tests in `os-url`.

---

## L. Geschwindigkeit, User-Agent, Header

### CRAWL-112 Parallele Threads
- **Priorität:** P0
- **Beschreibung:** Anzahl gleichzeitig laufender Abrufe.
- **Eingaben/Konfiguration:** Konfiguration › Geschwindigkeit › „Max. Threads“ (Standard: 5; Referenz-Default; Bereich 1–200, OS-Grenze). Zusätzlich OS-Erweiterung „Max. gleichzeitige Verbindungen pro Host“ (Standard: = Threads).
- **Akzeptanzkriterien:**
  - [ ] Threads 1: Fixture-Server sieht nie mehr als 1 gleichzeitige Anfrage.
  - [ ] Threads 5: höchstens 5 gleichzeitige Anfragen, bei ausreichend Arbeit mindestens 4 im Mittel (Auslastungstest mit 200 ms Verzögerung je Seite, 100 Seiten: Dauer ≤ 100 × 0,2 / 5 × 1,3 s).
  - [ ] Änderung während des Crawls wirkt innerhalb von 2 s ohne Neustart.
  - [ ] Ab 20 Threads zeigt die Oberfläche einen Hinweis zur Serverlast.
- **Testfixture:** `crawl-site-100` mit `delay_ms=200`; Server zählt gleichzeitige Verbindungen.

### CRAWL-113 Maximale URLs pro Sekunde
- **Priorität:** P0
- **Beschreibung:** Drosselt die Anfragerate unabhängig von der Thread-Zahl.
- **Eingaben/Konfiguration:** Geschwindigkeit › „Max. URLs/s begrenzen“ (Standard: aus) + Wert (Dezimalzahl, z. B. 0,5 = eine Anfrage alle 2 s).
- **Akzeptanzkriterien:**
  - [ ] 2,0 URLs/s, 20 Seiten: Gesamtdauer ≥ 9,5 s; in keinem 1-s-Fenster mehr als 2 Requestanfänge (Token-Bucket ohne Burst > 1; OS-Festlegung).
  - [ ] 0,5 URLs/s: Abstand zwischen Requests ≥ 2 s.
  - [ ] Die Grenze gilt für alle Requests einschließlich Ressourcen und externer URLs; robots.txt-Abrufe zählen mit (OS-Festlegung).
  - [ ] Im JS-Modus zählen nur Engine-Abrufe, nicht die vom Browser nachgeladenen Ressourcen (diese werden aber ebenfalls durch ein eigenes Limit gedrosselt: gleiche Rate × 10; OS-Default).
  - [ ] Statusleiste zeigt die tatsächlich erreichte Rate (gleitender Mittelwert über 10 s).
- **Testfixture:** `crawl-site-100` (Request-Log mit Zeitstempeln).

### CRAWL-114 User-Agent und Voreinstellungen
- **Priorität:** P0
- **Beschreibung:** Der HTTP-User-Agent ist frei wählbar oder aus Voreinstellungen. Getrennt davon wird das Token festgelegt, unter dem robots.txt-Gruppen und agentenspezifische Meta-Robots ausgewertet werden.
- **Eingaben/Konfiguration:** Konfiguration › User-Agent: Voreinstellung (Liste), Feld „HTTP-User-Agent“, Feld „robots.txt-User-Agent“ (Token). Voreinstellungen mindestens: OpenSpider (Standard: `Mozilla/5.0 (compatible; OpenSpider/<Version>; +https://…)`, Token `OpenSpider`), Googlebot Smartphone, Googlebot Desktop, Googlebot Image, Googlebot News, Googlebot Video, Google-InspectionTool, Google-Extended (nur Token), Bingbot Mobil, Bingbot Desktop, Yandex, Baiduspider, DuckDuckBot, Applebot, GPTBot, ClaudeBot, PerplexityBot, Chrome Desktop, Chrome Mobil, Safari iPhone, Firefox, Edge, Benutzerdefiniert. Die Liste liegt als Datendatei vor und kann ohne Code-Änderung aktualisiert werden.
- **Akzeptanzkriterien:**
  - [ ] `/echo-headers` zeigt exakt den eingestellten UA-String.
  - [ ] Bei Voreinstellung „Googlebot Smartphone“ wird als robots-Token `Googlebot` verwendet; die Gruppe `User-agent: Googlebot` der Fixture greift.
  - [ ] Ein frei eingegebener UA ohne gesetztes Token verwendet das Token `*`-Fallback und eine Warnung erscheint.
  - [ ] Derselbe UA wird für Rendering, Formular-Login und Sitemaps verwendet.
  - [ ] Chrome-Version in Browser-Voreinstellungen entspricht der mitgelieferten Chromium-Version.
- **Testfixture:** `crawl-http-basic/echo-headers`, `crawl-robots` (Googlebot-Gruppe).

### CRAWL-115 Eigene HTTP-Request-Header
- **Priorität:** P1
- **Beschreibung:** Beliebige zusätzliche Request-Header (Name/Wert) werden jeder Anfrage hinzugefügt, z. B. Bot-Authentifizierung, Bypass-Tokens, `Referer`, `Cookie`, `X-Forwarded-For`. Voreinstellungen für häufige Header-Namen (u. a. `Accept-Language`, `Cookie`, `Referer`, `Authorization`, Header-Namen für Bot-Authentifizierung von Shop-Plattformen und Web-Bot-Auth-Signaturen) erleichtern die Eingabe.
- **Eingaben/Konfiguration:** Konfiguration › HTTP-Header: Tabelle (Name, Wert); optional Einschränkung auf Host (OS-Erweiterung, Standard: alle Hosts der Crawl-Domain; extern: nicht senden).
- **Akzeptanzkriterien:**
  - [ ] `/echo-headers` enthält jeden konfigurierten Header genau einmal.
  - [ ] Ein eigener `User-Agent`-Eintrag wird abgewiesen mit Hinweis auf CRAWL-114.
  - [ ] Ein eigener `Cookie`-Header wird mit Cookies aus dem Cookie-Speicher zusammengeführt (nicht überschrieben).
  - [ ] Header mit Zeilenumbrüchen im Wert werden abgewiesen (Header-Injection).
  - [ ] Standardmäßig werden eigene Header nicht an externe Hosts gesendet (Schutz von Tokens; OS-Festlegung, siehe Q).
  - [ ] Der Renderer sendet dieselben Header.
- **Testfixture:** `crawl-http-basic/echo-headers`, `other.test/echo-headers`.

### CRAWL-116 Accept-Language
- **Priorität:** P1
- **Beschreibung:** Sprach-Header für sprachabhängig ausliefernde Websites. Standardmäßig wird kein `Accept-Language` gesendet (wie Googlebot); er kann über eine Voreinstellung gesetzt werden.
- **Eingaben/Konfiguration:** HTTP-Header › Voreinstellung „Accept-Language“ mit Auswahlliste gängiger Sprach-Tags (z. B. `de-DE,de;q=0.9`) oder freiem Wert.
- **Akzeptanzkriterien:**
  - [ ] Ohne Konfiguration enthält `/echo-headers` keinen `Accept-Language`.
  - [ ] Mit `de-DE` liefert die Fixture-Route `/lang` die deutsche Fassung (Titel „Startseite“), sonst die englische („Home“).
  - [ ] Im Rendering wird die Browsersprache (`navigator.language`) passend gesetzt.
- **Testfixture:** `crawl-http-basic/lang` (wählt Inhalt anhand `Accept-Language`).

---

## M. Authentifizierung und Proxy

### CRAWL-117 Standard-Authentifizierung: Basic
- **Priorität:** P0
- **Beschreibung:** HTTP-Basic-Authentifizierung. Erhält die Engine 401 mit `WWW-Authenticate: Basic`, fragt die Oberfläche nach Zugangsdaten (Dialog mit Host und Realm) – oder verwendet vorab hinterlegte Daten.
- **Eingaben/Konfiguration:** Konfiguration › Authentifizierung › Standardbasiert: Tabelle (URL-Präfix bzw. Host, Benutzername, Passwort); Schalter „Bei Bedarf nachfragen“ (Standard: an; im Headless-Betrieb aus).
- **Akzeptanzkriterien:**
  - [ ] Mit hinterlegten Daten: geschützte Seite liefert 200; die Zugangsdaten werden nur an den passenden Host gesendet, nie an andere Hosts oder nach einer Weiterleitung auf einen fremden Host.
  - [ ] Ohne Daten und mit Nachfragen: Crawl pausiert die betroffenen URLs, Dialog erscheint einmal pro Host/Realm; nach Eingabe werden alle wartenden URLs fortgesetzt; „Abbrechen“ speichert die 401-Antworten.
  - [ ] Headless ohne Daten: 401 wird gespeichert, kein Blockieren.
  - [ ] Benutzernamen und Passwörter mit Nicht-ASCII-Zeichen werden UTF-8-kodiert (Referenz-Fix 20.3).
  - [ ] Passwörter werden im Betriebssystem-Schlüsselbund gespeichert, nicht im Klartext im Profil.
  - [ ] Basic-Daten werden über unverschlüsseltes HTTP nur nach Bestätigung gesendet (OS-Festlegung).
- **Testfixture:** `crawl-auth`: `basic.site.test` (Realm „Test“, Benutzer `prüfer`/`geheim`), Weiterleitung von dort auf `other.test/echo-headers`.

### CRAWL-118 Standard-Authentifizierung: Digest
- **Priorität:** P1
- **Beschreibung:** HTTP-Digest nach RFC 7616 (MD5, SHA-256, `qop=auth`).
- **Eingaben/Konfiguration:** wie CRAWL-117 (Verfahren wird aus der Server-Challenge erkannt).
- **Akzeptanzkriterien:**
  - [ ] Digest MD5 und SHA-256 mit `qop=auth` liefern 200.
  - [ ] Nonce-Wiederverwendung mit `nc`-Zähler; `stale=true` führt zu einem neuen Versuch ohne Nutzerabfrage.
- **Testfixture:** `crawl-auth`: `digest-md5.site.test`, `digest-sha256.site.test`.

### CRAWL-119 NTLM und Negotiate/Kerberos
- **Priorität:** P2
- **Beschreibung:** Für Intranet-Sites. Die öffentliche Referenzdoku nennt nur Basic, Digest und Formular-Login; NTLM/Negotiate ist eine OpenSpider-Erweiterung für Unternehmensumgebungen.
- **Eingaben/Konfiguration:** wie CRAWL-117, zusätzlich Feld „Domäne“ (NTLM); Schalter „Anmeldedaten des angemeldeten Benutzers verwenden“ (Negotiate; Windows: SSPI, macOS: GSS-API mit vorhandenem Kerberos-Ticket). Das Verfahren wird aus der `WWW-Authenticate`-Challenge gewählt; bietet der Server mehrere an, gilt die Reihenfolge Negotiate > NTLM > Digest > Basic (OS-Festlegung).
- **Ausgabe:** Spalte `Auth-Verfahren` (OS-Erweiterung, Detailbereich HTTP-Header): `Basic`, `Digest`, `NTLM`, `Negotiate` bzw. leer.
- **Akzeptanzkriterien:**
  - [ ] NTLMv2-Handshake (Typ-1 → Typ-2 → Typ-3) gegen den Fixture-Server liefert 200; alle drei Nachrichten laufen über **dieselbe** TCP-Verbindung (Verbindungs-ID im Request-Log identisch).
  - [ ] Falsches Passwort → 401 wird gespeichert, kein Endlos-Wiederholen (höchstens 1 Handshake je URL); im Nachfrage-Modus erscheint der Zugangsdaten-Dialog einmal je Host.
  - [ ] Nach erfolgreichem Handshake werden weitere URLs desselben Hosts auf der authentifizierten Verbindung ohne erneuten Handshake abgerufen, solange die Verbindung offen ist (Request-Log: 10 Seiten = 1 Handshake bei 1 Thread).
  - [ ] Server bietet `Negotiate, NTLM` an, Schalter „angemeldeter Benutzer“ aus und NTLM-Daten hinterlegt: NTLM wird verwendet.
  - [ ] NTLMv1 wird nicht angeboten (Sicherheit); ein Server, der nur v1 akzeptiert, liefert 401.
  - [ ] Negotiate/Kerberos: manueller Test gegen eine Windows-Domäne nach Checkliste im Testplan (nicht automatisierbar) – Ergebnis 200 auf einer IIS-Seite mit „Windows-Authentifizierung“.
- **Testfixture:** `crawl-auth`: `ntlm.site.test` (Fixture implementiert NTLMv2-Server-Seite mit festem Benutzer `DOM\\prüfer`, verlangt Verbindungs-Persistenz), `ntlm-v1only.site.test`, `multi-auth.site.test` (`WWW-Authenticate: Negotiate` und `NTLM`).

### CRAWL-120 Formularbasierte Anmeldung (eingebetteter Browser)
- **Priorität:** P1
- **Beschreibung:** Für Login-Formulare: Der Nutzer öffnet die Login-Seite in einem eingebetteten Browserfenster und meldet sich manuell an. Die entstandenen Cookies sowie Local-/Session-Storage werden übernommen und für den Crawl verwendet (bei JS-Rendering auch im Renderer).
- **Eingaben/Konfiguration:** Authentifizierung › Formularbasiert: Tabelle (Login-URL, Status, Zeitpunkt der Anmeldung, Anzahl Cookies), „Hinzufügen“ öffnet den eingebetteten Browser mit der eingegebenen Login-URL; der Browser nutzt den konfigurierten User-Agent und Proxy. „OK“ im Browserfenster übernimmt den Zustand, „Abbrechen“ verwirft ihn. Zusätzlich Liste „Sitzungsschutz-Muster“ (OS-Erweiterung, siehe unten).
- **Ausgabe:** Status je Eintrag („Angemeldet, n Cookies“, „Abgelaufen“ – sobald das früheste Ablaufdatum der übernommenen Cookies überschritten ist, „Sitzung im Crawl verloren“).
- **Sitzungsschutz (eigene Herleitung):** Ein angemeldeter Crawler verhält sich wie ein Nutzer, der nacheinander jeden Link anklickt. Links, die eine Zustandsänderung auslösen (Abmelden, Löschen, Freigeben, Konto-Aktionen), werden dadurch tatsächlich ausgeführt – je nach Anwendung mit Datenverlust oder Sitzungsende. OpenSpider schützt deshalb aktiv statt nur zu warnen: Beim Hinzufügen einer Formular-Anmeldung wird die Liste „Sitzungsschutz-Muster“ vorbefüllt und als Exclude-Regeln (Vollübereinstimmung, `(?i)`) wirksam, solange mindestens eine Formular-Anmeldung aktiv ist. Vorbefüllung (OS-Liste, editierbar): `(?i).*(log-?out|sign-?out|abmelden|ausloggen).*`, `(?i).*(delete|remove|loeschen|löschen|entfernen).*`, `(?i).*/(admin|wp-admin|backend)/.*`, `(?i).*[?&](action|do)=(delete|remove|logout).*`.
- **Akzeptanzkriterien:**
  - [ ] Nach Login auf `form.site.test/login` liefert `/mitglieder/` im Crawl 200 statt 302 auf `/login`.
  - [ ] Local Storage und Session Storage aus dem Login werden beim Rendern wiederhergestellt: `/mitglieder/app` zeigt im gerenderten Titel „Token ok“.
  - [ ] Sitzungsschutz aktiv (Standard): `/logout` und `/konto/loeschen?id=1` erscheinen nicht im Request-Log; die Crawl-Übersicht zählt sie unter „Durch Sitzungsschutz übersprungen“; die Sitzung bleibt bis zum Crawl-Ende gültig (letzte gecrawlte `/mitglieder/`-Seite liefert 200).
  - [ ] Sitzungsschutz vom Nutzer geleert: vor dem Start erscheint eine Bestätigungsabfrage, die die Gefahr in einem Satz benennt; nach Bestätigung wird `/logout` angefragt und Folgeseiten liefern 302 auf `/login`.
  - [ ] Erkennt die Engine während des Crawls, dass die Sitzung verloren ging (≥ 3 Antworten mit Weiterleitung auf die Login-URL), meldet die Statusleiste „Sitzung verloren“ und der Eintrag erhält den Status „Sitzung im Crawl verloren“; der Crawl läuft weiter (OS-Festlegung).
  - [ ] Zwei-Faktor-Anmeldungen funktionieren, weil der Nutzer selbst im Browser handelt (Fixture `/login-2fa` mit festem Code).
  - [ ] Die übernommenen Cookies werden im Konfigurationsprofil nicht im Klartext gespeichert (nur über Auth-Profil CRAWL-121).
- **Testfixture:** `crawl-auth`: `form.site.test` mit `/login` (Formular, setzt Session-Cookie und `localStorage.token`), `/login-2fa`, `/mitglieder/` (prüft Cookie, verlinkt `/logout`, `/konto/loeschen?id=1` und 20 Unterseiten), `/mitglieder/app` (prüft Token per JS), `/logout` (löscht Session).

### CRAWL-121 Authentifizierungsprofile (verschlüsselt)
- **Priorität:** P2
- **Beschreibung:** Alle hinterlegten Zugangsdaten (standardbasiert und Formular-Cookies) können als verschlüsselte Datei exportiert werden, um sie in geplanten Crawls und der Kommandozeile zu verwenden.
- **Eingaben/Konfiguration:** Authentifizierung › Profile: „Exportieren“ (Passphrase), „Importieren“; CLI `--auth-config <Datei>` (+ Passphrase per Umgebungsvariable).
- **Ausgabe:** Datei `*.ospiderauth` (AES-256-GCM, Schlüssel per Argon2id aus Passphrase).
- **Akzeptanzkriterien:**
  - [ ] Datei enthält keine Klartext-Zugangsdaten (Test: Suche nach Passwort-Bytes negativ).
  - [ ] Falsche Passphrase → klare Fehlermeldung, kein Absturz.
  - [ ] Headless-Crawl mit importiertem Profil erreicht `/mitglieder/` mit 200 (solange Cookies gültig).
- **Testfixture:** `crawl-auth`.

### CRAWL-122 Proxy
- **Priorität:** P1
- **Beschreibung:** Alle HTTP(S)-Anfragen (inkl. Renderer) laufen über einen konfigurierten Proxy.
- **Paritätsabgleich (bewusste Abweichungen):** Die Referenz unterstützt genau **einen** Proxy-Server, **keine Ausnahmen** (entweder läuft der gesamte Verkehr über den Proxy oder keiner) und übernimmt Änderungen erst nach einem **Neustart der App**. OpenSpider ist eine Obermenge: Die Referenz-Konfiguration (ein HTTP-Proxy, keine Ausnahmen) ist vollständig abbildbar; zusätzlich gibt es SOCKS5, Ausnahmeliste, System-Proxy und Wirksamkeit ohne Neustart. Keine Referenz-Funktion fehlt.
- **Eingaben/Konfiguration:** Einstellungen › Proxy: Typ (HTTP, HTTPS, SOCKS5; SOCKS5 = OS-Erweiterung), Host, Port, optional Benutzername/Passwort (Schlüsselbund), Ausnahmeliste (OS-Erweiterung, Standard leer), „System-Proxy verwenden“ (OS-Erweiterung; Standardwert laut OUT-269, das diese Einstellung app-weit verbindlich festlegt). Genau ein Proxy je Typ-Auswahl (wie Referenz). Wirksam ab dem nächsten Crawl-Start (kein App-Neustart nötig, OS-Festlegung); ein laufender oder pausierter Crawl behält den Proxy, mit dem er gestartet wurde. Die Proxy-Einstellung ist app-weit (Crawl, Updates, Integrationen) und in OUT-269 beschrieben; dieser Eintrag legt nur das crawl-spezifische Verhalten fest.
- **Akzeptanzkriterien:**
  - [ ] Mit Proxy: Fixture-Proxy protokolliert jede Anfrage; Ziel-Fixture sieht die Proxy-IP.
  - [ ] HTTPS über HTTP-Proxy nutzt `CONNECT`.
  - [ ] Proxy-Authentifizierung (Basic) funktioniert; falsche Daten → Status „Proxy-Authentifizierung fehlgeschlagen“ (407) je URL.
  - [ ] Hosts in der Ausnahmeliste werden direkt angesprochen.
  - [ ] Proxy nicht erreichbar → klarer Fehler vor dem Start (Verbindungstest), nicht Tausende „Keine Antwort“-Zeilen.
  - [ ] Proxy-Änderung während eines pausierten Crawls: „Fortsetzen“ nutzt weiterhin den alten Proxy; ein neu gestarteter Crawl nutzt den neuen (Fixture-Proxy-Log), ohne dass die App neu gestartet wurde.
  - [ ] Paritätskonfiguration (HTTP-Proxy, leere Ausnahmeliste): ausnahmslos jede Anfrage einschließlich robots.txt, Sitemaps und Renderer-Ressourcen erscheint im Proxy-Log.
- **Testfixture:** `crawl-proxy`: Fixture-Proxy (HTTP CONNECT, SOCKS5, Basic-Auth) in `os-testsite`.

### CRAWL-123 Host-Auflösung überschreiben
- **Priorität:** P1 (wegen Testbarkeit aller Fixtures; im UI P2)
- **Beschreibung:** Zuordnung Hostname → IP-Adresse, die die DNS-Auflösung ersetzt (wie ein Eintrag in der Hosts-Datei). Nutzen: Staging-Server unter der Live-Domain crawlen; Grundlage aller Fixture-Tests mit `.test`-Domains.
- **Eingaben/Konfiguration:** Erweitert › „Host-Auflösung“: Tabelle (Host oder `*.domain`, IP); CLI `--resolve host=ip`.
- **Akzeptanzkriterien:**
  - [ ] `*.site.test → 127.0.0.1` lässt alle Fixture-Hosts lokal auflösen.
  - [ ] TLS-SNI und `Host`-Header verwenden weiterhin den Hostnamen.
  - [ ] Der Renderer verwendet dieselbe Zuordnung (Chromium-Host-Resolver-Regeln).
  - [ ] Ist eine Überschreibung aktiv, zeigt die Statusleiste ein Symbol mit Hinweis.
- **Testfixture:** alle Fixtures; zusätzlich Test, dass ohne Überschreibung `site.test` „DNS-Auflösung fehlgeschlagen“ liefert.

---

## N. Speicher, Persistenz, Steuerung

### CRAWL-124 Speichermodus: Datenbank vs. Arbeitsspeicher
- **Priorität:** P0
- **Beschreibung:** Das Referenzprodukt kennt einen Datenbankmodus (Standard, Daten auf SSD, unbegrenzt große Crawls, automatisch gespeichert) und einen Arbeitsspeichermodus (schneller, für kleinere Crawls, manuelles Speichern). OpenSpider schreibt laut Architektur immer in eine SQLite-Crawl-Datei und bildet den Unterschied über ein Leistungsprofil ab.
- **Eingaben/Konfiguration:** Einstellungen › Speicher: Speicherort für Crawls (Standard: Benutzer-Datenverzeichnis über `dirs`), Leistungsprofil „Ausgewogen“ (Standard; SQLite-Cache 256 MB) / „Maximale Geschwindigkeit“ (großer Cache, z. B. 25 % des RAM, synchrone Schreibvorgänge reduziert) / „Minimaler Speicher“ (Cache 64 MB); RAM-Obergrenze für den Prozess (Warnschwelle, CRAWL-129).
- **Ausgabe:** Einstellungsseite zeigt das aktive Profil und den Speicherort; Hilfe › Diagnose (CRAWL-129) zeigt den tatsächlich gesetzten SQLite-Cache.
- **Akzeptanzkriterien:**
  - [ ] Profil-Parameter sind überprüfbar: „Ausgewogen“ setzt `PRAGMA cache_size` auf −262144 (256 MiB) und `synchronous = NORMAL`; „Minimaler Speicher“ −65536 (64 MiB), `synchronous = NORMAL`; „Maximale Geschwindigkeit“ −(25 % des physischen RAM in KiB, höchstens 8 GiB), `synchronous = OFF` während des Crawls, `NORMAL` nach Abschluss. Test liest die PRAGMAs der offenen Crawl-Datei aus.
  - [ ] Ein Crawl von 1 000 000 URLs (synthetische Fixture) läuft mit Profil „Ausgewogen“ bei < 2 GB RSS durch.
  - [ ] Ein Crawl von 100 000 URLs ist mit „Maximale Geschwindigkeit“ mindestens so schnell wie mit „Ausgewogen“ (Benchmark im CI-Nightly; Toleranz 5 %).
  - [ ] Mit „Minimaler Speicher“ bleibt der RSS-Spitzenwert beim 1-Mio.-Crawl mindestens 20 % unter dem von „Ausgewogen“.
  - [ ] Alle Funktionen (Vergleich, Segmente, Verwaltung, Fortsetzen nach Absturz) sind in allen drei Profilen verfügbar (Test durchläuft CRAWL-032/125/134 je Profil).
  - [ ] Speicherort auf Netzlaufwerken (SMB/AFP/NFS), Sync-Ordnern (Pfade unter den bekannten Dropbox-, OneDrive-, iCloud-Drive-, Google-Drive-Wurzeln) oder Dateisystemen ohne zuverlässige Dateisperren (FAT32/exFAT) wird beim Auswählen erkannt: Dialog mit Warnung und den Optionen „Anderen Ordner wählen“ / „Trotzdem verwenden“; die Wahl wird gespeichert.
  - [ ] Fehlender Speicherplatz (< 1 GB frei auf dem Laufwerk des Speicherorts, geprüft alle 5 s) pausiert den Crawl automatisch mit Meldung „Speicherplatz knapp“; die Datei bleibt konsistent (`PRAGMA integrity_check` = ok).
  - [ ] Ändern des Speicherorts verschiebt bestehende Crawls nicht automatisch, sondern bietet „Vorhandene Crawls verschieben“ an; die Crawl-Verwaltung findet Crawls an beiden Orten bis zur Verschiebung.
- **Testfixture:** `crawl-synthetic` (Generator-Route `/gen/{n}` mit 10 Links je Seite, deterministisch, beliebig groß).

### CRAWL-125 Automatisches Speichern und Wiederherstellung
- **Priorität:** P0
- **Beschreibung:** Da jeder Crawl fortlaufend in seine Datei schreibt, gehen bei Absturz, Stromausfall oder versehentlichem Schließen keine Daten verloren. Beim nächsten Start wird ein unterbrochener Crawl erkannt und zum Fortsetzen angeboten.
- **Eingaben/Konfiguration:** —
- **Akzeptanzkriterien:**
  - [ ] Prozess wird während eines Crawls mit SIGKILL beendet: Beim nächsten Start ist der Crawl in der Verwaltung mit Status „Unterbrochen“ gelistet; alle bis ≤ 2 s vor dem Abbruch gecrawlten URLs sind vorhanden; Datei ist konsistent (`PRAGMA integrity_check` = ok).
  - [ ] „Fortsetzen“ eines unterbrochenen Crawls crawlt keine bereits gespeicherte URL erneut und vervollständigt die Warteschlange.
  - [ ] Neuer Crawl, Löschen der Ansicht oder Beenden der App sichern den laufenden Crawl ohne Rückfrage.
- **Testfixture:** `crawl-synthetic` (10 000 Seiten, Abbruch nach 5 s).

### CRAWL-126 Crawl pausieren und fortsetzen
- **Priorität:** P0
- **Beschreibung:** Ein laufender Crawl kann pausiert und später – auch nach Neustart der App oder auf einem anderen Rechner – fortgesetzt werden. Während der Pause können Konfiguration (Limits, Include/Exclude, Geschwindigkeit, Erfassungsschalter) geändert werden.
- **Eingaben/Konfiguration:** Schaltflächen „Pause“/„Fortsetzen“ (Tastenkürzel siehe UI-Bereich); CLI: erstes SIGINT/Strg+C pausiert, speichert und beendet den Prozess mit Exit-Code 130; zweites SIGINT innerhalb von 5 s erzwingt sofortiges Beenden ohne Abwarten laufender Anfragen (OS-Festlegung, Details OUT-206). Fortsetzen per CLI: `--load-crawl <ID> --resume` (OS-Erweiterung).
- **Ausgabe:** Status „Pausiert“ in der Statusleiste; Warteschlangenlänge; Zeitpunkt der Pause in der Crawl-Übersicht.
- **Änderbar während der Pause** (wirken auf noch nicht abgerufene URLs): Limits, Include/Exclude, URL-Umschreibung (nur für neu entdeckte URLs), Geschwindigkeit/Threads, Erfassungsschalter, Speichern/Crawlen-Schalter, User-Agent, Header, Authentifizierung. **Nicht änderbar** (Felder ausgegraut mit Tooltip „neuer Crawl nötig“): Modus, Start-URL, Crawl-Bereich, Rendering-Modus (CRAWL-077), Proxy (CRAWL-122).
- **Akzeptanzkriterien:**
  - [ ] Nach „Pause“ startet innerhalb von 1 s keine neue Anfrage mehr (Request-Log); laufende Anfragen werden zu Ende geführt und gespeichert (höchstens Threads-viele zusätzliche Zeilen).
  - [ ] Pausieren eines Crawls mit 1 Mio. Warteschlangeneinträgen dauert < 3 s bis zum Status „Pausiert“ (Referenz-Fix 19.5 zeigt, dass das relevant ist).
  - [ ] Fortsetzen nach App-Neustart: Gesamtergebnis identisch zu einem ununterbrochenen Crawl (Golden-Vergleich der URL-Menge, Statuscodes und Tiefen); keine URL im Request-Log doppelt.
  - [ ] Fortsetzen auf einem anderen Rechner: Crawl-Datei kopieren, öffnen, „Fortsetzen“ → gleiches Ergebnis wie oben.
  - [ ] Wird während der Pause ein Exclude ergänzt, werden passende Warteschlangeneinträge verworfen; der Zähler „verbleibend“ sinkt sofort nach „OK“ im Konfigurationsdialog.
  - [ ] Wird das URL-Limit erhöht, setzt „Fortsetzen“ fort; wird es unter die bereits gecrawlte Anzahl gesenkt, endet der Crawl beim Fortsetzen sofort mit Status „Limit erreicht“.
  - [ ] Threads von 5 auf 1 geändert: nach dem Fortsetzen sieht der Fixture-Server nie mehr als 1 gleichzeitige Anfrage.
  - [ ] Versuch, während der Pause den Rendering-Modus zu ändern: Feld ist ausgegraut; per Profil-Laden erzwungene Änderung wird mit Meldung abgewiesen.
  - [ ] Während einer Pause können API-Daten angefordert werden, ohne den Crawl fortzusetzen (Schnittstelle für den API-Bereich).
  - [ ] CLI: SIGINT nach 30 URLs → Exit-Code 130, Crawl in der Verwaltung mit Status „Pausiert“; `--load-crawl <ID> --resume` führt ihn bis zum Ende.
- **Testfixture:** `crawl-site-100` mit `delay_ms=100`; Pause nach 30 URLs, Neustart, Fortsetzen; Variante mit Exclude-Ergänzung `.*/bereich-b/.*` während der Pause.

### CRAWL-127 Crawl speichern, öffnen, importieren, exportieren
- **Priorität:** P0
- **Beschreibung:** Crawls liegen als `*.ospider`-Dateien vor und können geöffnet, unter neuem Namen gespeichert, als Datei exportiert (für Weitergabe) und importiert werden. Die Konfiguration des Crawls ist in der Datei enthalten.
- **Eingaben/Konfiguration:** Menü Datei › Öffnen, Zuletzt geöffnet (10 Einträge, neueste oben, nicht mehr vorhandene Dateien werden beim Anklicken mit Hinweis entfernt), Speichern unter, Exportieren (Kopie der Crawl-Datei an frei wählbaren Ort, ohne den Crawl aus der Verwaltung zu entfernen), Importieren (Kopie in den Speicherort und Aufnahme in die Verwaltung); Doppelklick auf `.ospider` im Dateimanager öffnet die App (Dateizuordnung macOS/Windows, OUT-232). Format- und Migrationsdetails: OUT-230, OUT-226.
- **Akzeptanzkriterien:**
  - [ ] Speichern und Öffnen eines Crawls ergibt identische Tabelleninhalte (Golden-Vergleich aller Tabs, aller Filter, aller Detailbereiche).
  - [ ] Export → Import auf einem zweiten Rechner (anderes Betriebssystem: macOS → Windows) ergibt identische Tabelleninhalte; die importierte Datei erhält eine neue Datenbank-ID, wenn die ID dort bereits existiert.
  - [ ] Öffnen einer Datei mit 1 Mio. URLs zeigt die erste Tabellenseite in < 3 s (Referenzmaschine: 8 Kerne, SSD).
  - [ ] Dateien einer älteren Schema-Version werden beim Öffnen migriert; vorher wird eine Sicherheitskopie `<name>.v<alt>.bak.ospider` angelegt; Dateien einer neueren Schema-Version werden mit der Meldung „Mit OpenSpider ≥ x.y erstellt – bitte aktualisieren“ abgelehnt, ohne die Datei zu verändern (Prüfsumme vorher = nachher).
  - [ ] Ein geöffneter Crawl enthält die damalige Konfiguration; „Konfiguration übernehmen“ macht sie zur aktuellen; ohne diese Aktion bleibt die aktuelle Konfiguration unverändert.
  - [ ] Eine beschädigte Datei (abgeschnitten nach 50 %) oder eine fremde Datei (umbenannte PNG) führt zu einer Fehlermeldung mit Dateiname und Ursache („keine OpenSpider-Datei“ / „Datei beschädigt“), nicht zu einem Absturz; die App bleibt bedienbar.
  - [ ] Eine bereits geöffnete Datei wird beim zweiten Öffnen nur in den Vordergrund geholt, nicht doppelt geöffnet.
  - [ ] Import von Crawl-Dateien des Referenzprodukts ist **nicht** Teil der Spezifikation (proprietäres Format; siehe Q21).
- **Testfixture:** `crawl-site-100`; beschädigte Datei `tests/fixtures/crawl-files/corrupt.ospider` (abgeschnitten), `not-a-crawl.ospider` (PNG-Inhalt), `schema-v1.ospider`, `schema-v99.ospider` (künstlich hohe Version).

### CRAWL-128 Crawl-Verwaltung (Projekte, Sperren, Aufbewahrung)
- **Priorität:** P1
- **Beschreibung:** Übersicht aller gespeicherten Crawls mit Ordnung in Projekte, Umbenennen, Löschen, Sperren und automatischer Aufbewahrungsregel.
- **Eingaben/Konfiguration:** Datei › Crawls: Tabelle mit Spalten `Projekt`, `Name`, `Modus`, `Start-URL`, `Gestartet`, `Dauer`, `URLs`, `Größe auf Datenträger`, `Status` (Abgeschlossen / Pausiert / Unterbrochen / Fehlgeschlagen; Export-Schlüssel wie OUT-228), `Gesperrt`, `Datenbank-ID` `crawl_id`. Kontextmenü und Schaltflächen siehe CRAWL-143. Filter „Anzeigen: Crawls / Vergleiche / Alle“; Suche (Strg/Cmd+F) über Name, Projekt, Start-URL. Aufbewahrungsregel: Einstellungen › Crawl-Aufbewahrung, Werte und Bezugszeitpunkt verbindlich in OUT-229 (Standard „Nie“). Dieser Eintrag ist die crawl-seitige Sicht auf OUT-228/229; bei Widerspruch gilt OUT.
- **Akzeptanzkriterien:**
  - [ ] Die Spalte `Datenbank-ID` enthält eine stabile, eindeutige Kennung (UUID v7, OS-Festlegung), die sich durch Umbenennen, Verschieben in ein Projekt oder Sperren nicht ändert; Duplizieren vergibt eine neue ID.
  - [ ] Die per Kontextmenü kopierte ID ist byte-identisch mit dem Wert in `--list-crawls` und wird von `--load-crawl <ID>` akzeptiert (End-to-End-Test: UI-Zwischenablage → CLI).
  - [ ] Gesperrte Crawls und alle Crawls gesperrter Projekte werden von der Aufbewahrungsregel nie gelöscht; auch nach dem Sperren neu angelegte Crawls im gesperrten Projekt sind geschützt.
  - [ ] Aufbewahrung „1 Woche“: ein Crawl mit Ende vor 8 Tagen wird beim nächsten App-Start gelöscht, einer mit Ende vor 6 Tagen bleibt (Unit-Test mit manipulierbarer Uhr).
  - [ ] Gesperrte Crawls lassen sich nicht löschen (Kontextmenü „Löschen“ ausgegraut, Tooltip „gesperrt“).
  - [ ] Löschen eines Crawls, der in einem gespeicherten Vergleich verwendet wird, zeigt vorher eine Warnung mit dem Namen des Vergleichs.
  - [ ] Mehrfachauswahl (Umschalt/Strg-Klick) erlaubt Löschen, Sperren, Verschieben und Exportieren von bis zu 1 000 Crawls in einem Schritt.
  - [ ] Die Liste lädt bei 1 000 Crawls in < 1 s (Metadaten in separatem Index, nicht durch Öffnen jeder Datei); die Suche filtert bei 1 000 Einträgen in < 100 ms.
  - [ ] Status-Abbildung: ein per SIGKILL abgebrochener Crawl erscheint als „Unterbrochen“ (CRAWL-125), ein per Pause/SIGINT beendeter als „Pausiert“.
- **Testfixture:** Unit-/Integrationstest in `os-store` mit generierten Crawl-Dateien (3 Projekte, eines gesperrt), manipulierten Zeitstempeln und einem Vergleich, der zwei davon referenziert.

### CRAWL-129 Speicherwarnung und Ressourcenschutz
- **Priorität:** P1
- **Beschreibung:** Überwachung von Arbeitsspeicher und Speicherplatz. Bei Engpässen wird der Crawl automatisch pausiert, statt abzustürzen.
- **Eingaben/Konfiguration:** Einstellungen › Speicher › „RAM-Obergrenze“ (OS-Default: 50 % des physischen RAM, höchstens physischer RAM − 2 GB, mindestens 1 GB; Eingabe in GB mit einer Nachkommastelle). Gemessen wird der RSS-Wert des Engine-Prozesses **plus** aller Renderer-Prozesse (Chromium) zusammen, Messintervall 1 s. Plattenplatz-Schwelle aus CRAWL-124 (1 GB). Zusammenhang mit PERF-001 (`07-performance.md`): dort die nicht-funktionale Obergrenze, hier die Laufzeitreaktion.
- **Ausgabe:** Warnbanner „Wenig Arbeitsspeicher – Crawl pausiert“ mit drei konkreten Hinweisen (Threads reduzieren, Rendering abschalten, HTML-Speicherung abschalten) und je einem Link direkt zur Einstellung; Eintrag im Log mit Messwert.
- **Akzeptanzkriterien:**
  - [ ] Stufe 1: Ab 80 % der Obergrenze erscheint ein gelber Hinweis in der Statusleiste („Arbeitsspeicher 80 %“), der Crawl läuft weiter.
  - [ ] Stufe 2: Ab 90 % der Obergrenze wird der Crawl innerhalb von 2 s pausiert (keine neuen Requests im Request-Log) und das Banner angezeigt (Test mit Obergrenze 300 MB und `crawl-synthetic`).
  - [ ] „Fortsetzen“ bei weiterhin ≥ 90 % pausiert erneut sofort und zeigt das Banner wieder; nach Senken der Threads und Speicherfreigabe < 80 % läuft der Crawl weiter.
  - [ ] Die Warnung erscheint in allen Phasen: Crawl, Crawl-Analyse (Referenz-Fix 23.2), Export, Vergleichsberechnung; während der Analyse wird diese pausiert statt abgebrochen.
  - [ ] Renderer-Prozesse werden mitgezählt: JS-Crawl mit künstlich speicherhungriger Seite (`/mem-hog.html` alloziert 200 MB) löst Stufe 2 aus, obwohl der Engine-Prozess allein darunter liegt.
  - [ ] Hilfe › Diagnose zeigt aktuelle RAM-Nutzung (Engine, Renderer, Summe), Obergrenze, Crawl-Dateigröße, freien Speicherplatz, aktives Leistungsprofil (CRAWL-124).
  - [ ] Ein Speicherengpass führt nie zu einem Prozessabsturz ohne vorherige Pause (Stresstest: 10 Läufe mit Obergrenze 300 MB, 0 Abstürze, alle Crawl-Dateien konsistent).
- **Testfixture:** `crawl-synthetic` mit Obergrenze 300 MB; `crawl-js/mem-hog.html`.

### CRAWL-130 Fortschritt und Crawl-Statistik
- **Priorität:** P0
- **Beschreibung:** Während und nach dem Crawl werden Fortschritt und Kennzahlen angezeigt.
- **Eingaben/Konfiguration:** —
- **Ausgabe:** Statusleiste: Modus, Status (Läuft/Pausiert/Abgeschlossen/Limit erreicht/Unterbrochen), gecrawlte URLs, verbleibende URLs (Warteschlange), Fortschritt in %, aktuelle Rate (URLs/s), mittlere Antwortzeit, verstrichene Zeit, geschätzte Restzeit, Startzeitpunkt, aktive Threads; aufklappbare Detailansicht. Dieselben Werte im Bericht „Crawl-Übersicht“ und im CLI-Fortschritt.
- **Definitionen:** „gecrawlt“ = Anzahl abgeschlossener Abrufe mit Ergebnis (inkl. Statuscode 0), ohne robots.txt-Abrufe; „Warteschlange“ = eingeplante, noch nicht gestartete URLs; „Rate“ = gleitender Mittelwert der abgeschlossenen Abrufe über die letzten 10 s; „mittlere Antwortzeit“ = arithmetisches Mittel von `Antwortzeit` (CRAWL-022) aller bisher gecrawlten URLs; „Restzeit“ = Warteschlange / Rate.
- **Akzeptanzkriterien:**
  - [ ] Zähler „gecrawlt“ entspricht nach Abschluss exakt der Anzahl Zeilen mit nicht leerem Statuscode über die Tabs „Intern“ + „Extern“ (Abgleich per Datenbankabfrage).
  - [ ] Prozent = gecrawlt / (gecrawlt + Warteschlange) × 100, ganzzahlig abgerundet; ist ein URL-Limit gesetzt und gecrawlt / Limit größer, wird dieser Wert verwendet; nach Abschluss immer 100.
  - [ ] Bei `crawl-site-100` mit 1 Thread und `delay_ms=100` zeigt die Rate nach 20 s einen Wert zwischen 7 und 10 URLs/s.
  - [ ] Restzeit wird erst nach 30 s Laufzeit **und** mindestens 100 gecrawlten URLs angezeigt (vorher „–“); bei Rate 0 (Pause) zeigt sie „–“.
  - [ ] Status wechselt korrekt: Läuft → Pausiert (Pause) → Läuft (Fortsetzen) → Abgeschlossen; bei Erreichen eines Limits „Limit erreicht“; nach Absturz und Neustart „Unterbrochen“.
  - [ ] UI-Aktualisierung höchstens 10× pro Sekunde (Architektur-Vorgabe); kein Einbruch der Crawl-Rate durch die Oberfläche (Crawl mit und ohne geöffnete UI unterscheidet sich < 10 % in der Dauer).
  - [ ] Die Werte der Statusleiste, des Berichts „Crawl-Übersicht“ und des CLI-Fortschritts (`--json-progress`, Felder `crawled`, `queued`, `percent`, `rate`, `avg_response_time`, `elapsed_s`, `eta_s`, `state`) sind zum selben Zeitpunkt identisch.
- **Testfixture:** `crawl-site-100` mit `delay_ms=100`.

### CRAWL-131 Einzelne URLs neu crawlen, entfernen, hinzufügen
- **Priorität:** P1
- **Beschreibung:** Per Kontextmenü können markierte URLs erneut abgerufen („Neu crawlen“) oder aus dem Crawl entfernt werden; über die Listenfunktion können während/nach einem Spider-Crawl zusätzliche URLs eingereiht werden.
- **Eingaben/Konfiguration:** Kontextmenü „Neu crawlen“, „Entfernen“; Menü „URLs hinzufügen“.
- **Akzeptanzkriterien:**
  - [ ] „Neu crawlen“ ersetzt alle Daten der URL (Status, Inhalte, Outlinks); neue Outlinks werden eingereiht, wenn sie im Bereich liegen; entfallene Outlinks werden entfernt und Inlink-Zähler anderer URLs angepasst.
  - [ ] Mehrfachauswahl von 1 000 URLs wird gebündelt verarbeitet.
  - [ ] „Entfernen“ löscht die URL und ihre ausgehenden Links; Inlinks auf sie bleiben als Links ohne Statuscode bestehen.
  - [ ] Nach diesen Aktionen wird ein Hinweis angezeigt, dass die Crawl-Analyse veraltet ist (UI-082); ist „Automatisch am Crawl-Ende analysieren“ (CRAWL-141) an, wird sie bei „Neu crawlen“ nicht automatisch erneut gestartet (OS-Festlegung, Nutzer startet manuell).
- **Testfixture:** `crawl-site-100` mit umschaltbarer Seite `/wechsel` (liefert beim 2. Abruf andere Links und 404).

### CRAWL-132 Konfigurationsprofile
- **Priorität:** P0
- **Beschreibung:** Die gesamte Crawl-Konfiguration kann als Profil gespeichert, geladen und als Standard festgelegt werden. Systemweite Einstellungen (Sprache, Speicherort, Proxy, Aufbewahrung, Design) gehören nicht zum Profil.
- **Eingaben/Konfiguration:** Konfiguration › Profile: Speichern unter, Laden, Zuletzt geladen, „Aktuelle Konfiguration als Standard speichern“, „Standard zurücksetzen“; ein einheitlicher Konfigurationsdialog mit Navigationsleiste und Schaltflächen OK/Abbrechen (Abbrechen verwirft alle Änderungen seit dem Öffnen).
- **Ausgabe:** Datei `*.ospiderconfig` (JSON, versioniert, menschenlesbar).
- **Akzeptanzkriterien:**
  - [ ] Speichern → Laden ergibt eine identische Konfiguration (Round-Trip-Test über alle Optionen).
  - [ ] Ein Profil ohne einzelne Schlüssel lädt fehlende Werte mit Standardwerten (Vorwärtskompatibilität); unbekannte Schlüssel werden ignoriert und gemeldet.
  - [ ] Passwörter sind nicht im Profil enthalten (nur Verweis auf Schlüsselbund bzw. Auth-Profil, CRAWL-121).
  - [ ] „Standard zurücksetzen“ stellt die Werkseinstellungen wieder her.
  - [ ] Das Profil kann von CLI (`--config`) und geplanten Crawls verwendet werden.
  - [ ] Segmente, eigene robots.txt, Include/Exclude, Umschreibungen, CDNs, Header werden im Profil gespeichert.
- **Testfixture:** Unit-Test in `os-model` (Round-Trip), Profil-Datei `tests/fixtures/crawl-config/minimal.ospiderconfig`.

### CRAWL-133 Konfigurations-Diff zum Standard
- **Priorität:** P2
- **Beschreibung:** Ein Fenster listet alle Optionen, die von den Werkseinstellungen abweichen – nützlich zur Fehlersuche bei unerwartetem Crawl-Verhalten.
- **Eingaben/Konfiguration:** Tastenkürzel Strg/Cmd+Umschalt+C oder Menü Konfiguration › „Abweichungen anzeigen“.
- **Ausgabe:** Tabelle (Option, Standardwert, Aktueller Wert).
- **Akzeptanzkriterien:**
  - [ ] Nach Änderung von Threads 5 → 10 zeigt das Fenster genau diese eine Zeile.
  - [ ] Kopieren als Text ist möglich.
- **Testfixture:** Unit-Test.

### CRAWL-134 Segmente
- **Priorität:** P1
- **Beschreibung:** Benannte, farbige Gruppen von URLs anhand von Regeln (z. B. „Produktseiten“, „Blog“, „Wichtige Seiten“). Jede URL erhält höchstens ein Segment: Segmente werden in der Reihenfolge der Liste geprüft, das erste passende gewinnt. Segmente können sich auf andere Segmente beziehen; Abhängigkeiten müssen weiter oben stehen. Sie können vor, während oder nach dem Crawl angelegt werden.
- **Eingaben/Konfiguration:** Konfiguration › Segmente: Liste (Name, Farbe, Reihenfolge); je Segment Regelgruppen: Bedingungen innerhalb einer Gruppe UND-verknüpft, Gruppen untereinander ODER-verknüpft. Bedingung = Feld (beliebige Spalte des Crawls, inkl. Adresse, Statuscode, Indexierbarkeit, Crawl-Tiefe, Inhaltstyp, Wortanzahl, API-Daten, Nachlauf-Analysewerte, Issues, anderes Segment) + Operator (enthält, enthält nicht, ist gleich, ist nicht gleich, beginnt mit, endet mit, Regex passt, Regex passt nicht, größer als, kleiner als, größer gleich, kleiner gleich, ist leer, ist nicht leer, hat Issue, hat Issue nicht, ist in Segment) + Wert.
- **Ausgabe:** Spalte `Segment` (farbiges Label) in allen Tabs; Segment-Filter in jeder Tabelle; rechter Bereich „Segmente“ mit Aggregaten je Segment (URLs, indexierbar/nicht indexierbar, Issues, Ansicht „nach Crawl-Tiefe“ etc.); Segment-Leiste im Issues-Bereich; Nutzung in Visualisierungen, Sitemap-Erzeugung je Segment, Berichten.
- **Operator-Semantik:** Textoperatoren (enthält, ist gleich, beginnt/endet mit) vergleichen case-sensitiv (OS-Festlegung; `(?i)` über den Regex-Operator für case-insensitiv); numerische Operatoren nur auf numerischen Feldern, Leerwerte erfüllen keinen numerischen Vergleich; „Regex passt“ = Suche (Treffer irgendwo, siehe Konventionen); „ist leer“ trifft auf fehlende und leere Werte zu; Felder mit mehreren Werten (z. B. `Titel 1`, `Titel 2`) erfüllen eine Bedingung, wenn mindestens ein Wert sie erfüllt.
- **Akzeptanzkriterien:**
  - [ ] Regel „Adresse enthält `/produkt/`“ auf `crawl-shop`: genau die 40 Produkt-URLs tragen das Segment (Zähler im Segment-Bereich = 40).
  - [ ] Zwei überlappende Segmente („Shop“: Adresse enthält `/produkt/`; „Alle Seiten“: Inhaltstyp enthält `html`): Produkt-URLs erhalten „Shop“, wenn „Shop“ oben steht; nach Tausch der Reihenfolge erhalten sie „Alle Seiten“.
  - [ ] UND/ODER: Gruppe 1 = {Adresse enthält `/produkt/` UND Statuscode = 404}, Gruppe 2 = {Adresse enthält `/blog/`}: Segment enthält genau die 404-Produktseiten plus alle Blogseiten.
  - [ ] Regel „Statuscode größer als 399“ funktioniert numerisch (404 und 500 ja, 200 und 301 nein; nicht lexikalisch; Referenz-Fix 21.2); Zeilen ohne Statuscode erfüllen sie nicht.
  - [ ] Regex-Operator verwendet den gemeinsamen Dialekt; eine ungültige Regex wird beim Speichern mit Zeile und Position gemeldet und das Segment nicht gespeichert (Referenz-Fix 19.1).
  - [ ] Bedingung „ist in Segment Shop“ in Segment „Shop-Fehler“ (unterhalb von „Shop“) funktioniert; dieselbe Bedingung in einem Segment **oberhalb** von „Shop“ wird beim Speichern als ungültig markiert.
  - [ ] Änderung der Segmente nach dem Crawl aktualisiert alle Ansichten ohne Neucrawl (Request-Zähler unverändert) in < 5 s bei 100 000 URLs.
  - [ ] Segmente, die vor dem Crawl angelegt wurden, sind während des Crawls an neuen Zeilen sofort sichtbar (≤ 1 s nach Erscheinen der Zeile).
  - [ ] Segmente werden im Profil und in der Crawl-Datei gespeichert; Öffnen eines Crawls zeigt die damals gespeicherten Segmente, „Konfiguration übernehmen“ (CRAWL-127) übernimmt sie ins aktuelle Profil.
  - [ ] URLs ohne passendes Segment haben ein leeres Segment und sind über den Filter „Kein Segment“ erreichbar; Summe aller Segmentzähler + „Kein Segment“ = Anzahl interner URLs.
  - [ ] Bedingungen auf Werte der Crawl-Analyse (z. B. Link Score) sind vor Abschluss der Analyse (CRAWL-141) als „ausstehend“ gekennzeichnet und weisen der URL kein Segment zu.
- **Testfixture:** `crawl-shop` (40 Produktseiten, davon 3 mit 404, 10 Kategorieseiten, 10 Blogseiten, 1 Seite mit 500) mit vordefiniertem Segment-Profil `segments.ospiderconfig`.

### CRAWL-135 Kommandozeile / Headless-Betrieb
- **Priorität:** P1
- **Beschreibung:** Das Binary `openspider` führt Crawls ohne Oberfläche aus – für Server, Automatisierung und geplante Crawls. Verbindliche Einzelspezifikation jedes Flags (inkl. Exit-Codes) liegt im Export-/Betriebsbereich (OUT-180 bis OUT-209); dieser Eintrag legt den crawl-seitigen Mindestumfang fest und hält die Referenz-Syntax fest, damit vorhandene Skripte mit minimaler Anpassung übertragbar sind.
- **Eingaben/Konfiguration – Referenz-Flags (Syntax wie Referenz):**
  - Crawl-Quelle: `--crawl <URL>` (Spider-Modus); `--crawl-list <Datei|Sitemap-URL>` (Listenmodus; eine XML-Sitemap bzw. ein Sitemap-Index wird – wie in der Referenz – über `--crawl-list` übergeben, OUT-183); `--crawl-google-sheet <Google-Konto> <Tabellen-URL>` (Listenmodus aus Google-Tabelle; in der Referenz laut Release-Notes 22/23 vorhanden, offizielle Syntax nicht in der Handbuchseite gefunden – Q31; Integration im API-Bereich).
  - Konfiguration: `--config <Profil>`, `--auth-config <Datei>`, `--headless`, `--save-crawl`, `--output-folder <Pfad>`, `--timestamped-output`, `--overwrite`, `--project-name <Name>`, `--task-name <Name>`.
  - Gespeicherte Crawls: `--load-crawl <Datei|Datenbank-ID>`, `--list-crawls`.
  - Vergleich: `--crawl-comparison <Datenbank-ID> <Datenbank-ID>`; `--project-crawl-comparison "true"` (nur zusammen mit `--project-name`; vergleicht die beiden jüngsten Crawls des Projekts, CRAWL-036).
  - APIs (Verweis, Spezifikation in OUT-201 bzw. API-Bereich `04-custom-integrations.md`): `--use-google-analytics-4 "<Konto>" "<Account>" "<Property>" "<Datenstrom>"`, `--use-google-search-console "<Konto>" "<Website>"`, `--use-pagespeed`, `--use-majestic`, `--use-mozscape`, `--use-ahrefs`.
  - Benachrichtigung: `--email-on-complete <Adresse[,Adresse…]>` (OUT-199).
  - Exporte (Verweis OUT-192 ff.): `--export-tabs`, `--bulk-export`, `--save-report`, `--export-format`, `--create-sitemap`, `--create-images-sitemap`.
  - Hilfe: `--help`, `--help <Unterthema>`, `--version`.
- **Eingaben/Konfiguration – OS-Erweiterungen (in der Referenz nicht vorhanden, so gekennzeichnet):** `--crawl-sitemap <URL>` (Kurzform für `--crawl-list <Sitemap-URL>`, verhält sich identisch; reine Bequemlichkeit), `--set <schlüssel>=<wert>` (Einzeloption des Profils überschreiben, mehrfach), `--resolve <host>=<ip>` (CRAWL-123), `--max-urls <n>` (Kurzform für `--set limits.max_urls=<n>`), `--resume` (mit `--load-crawl`, CRAWL-126), `--quiet`, `--json-progress` (Fortschritt als JSON-Zeilen auf stdout, Felder siehe CRAWL-130).
- **Ausgabe:** Exit-Codes verbindlich nach OUT-204; Kurzfassung: 0 = Erfolg, 1 = Konfigurations-/Aufruffehler, 2 = Crawl-Fehler (z. B. Start-URL nicht erreichbar, unbekannte ID), 3 = Exportfehler, 130 = durch Nutzer abgebrochen (Crawl gespeichert).
- **Akzeptanzkriterien:**
  - [ ] `openspider --crawl https://site.test/ --headless --save-crawl --output-folder X --resolve '*.site.test=127.0.0.1'` erzeugt eine `.ospider`-Datei mit identischem Ergebnis (URL-Menge, Statuscodes, Tiefen) wie der UI-Crawl derselben Fixture mit demselben Profil.
  - [ ] `--crawl-list https://site.test/sitemap_index.xml` und `--crawl-sitemap https://site.test/sitemap_index.xml` erzeugen identische Crawls (gleiche URL-Menge, gleiche Reihenfolge im Request-Log).
  - [ ] `--crawl-list urls.txt` mit Fixture `dupes.txt` gibt auf stderr dieselben Zähler aus wie der Zusammenfassungsdialog in CRAWL-027.
  - [ ] Ohne `--timestamped-output` und mit nicht leerem Ausgabeordner ohne `--overwrite`: Abbruch mit Exit 1 und Meldung, welche Datei überschrieben würde (statt stiller Leerausgabe).
  - [ ] `--load-crawl <ID>` öffnet einen Crawl aus der Verwaltung ohne Netzwerkzugriff; `--list-crawls` listet je Zeile `Datenbank-ID`, Name, Projekt, Startzeit, URLs, Status; die IDs sind identisch mit „Datenbank-ID kopieren“ in der UI (CRAWL-143).
  - [ ] `--project-crawl-comparison "true"` ohne `--project-name` → Exit 1; `--project-crawl-comparison MeinProjekt` (Projektname als Wert) → Exit 1 mit Hinweis auf die richtige Syntax.
  - [ ] `--use-pagespeed` ohne hinterlegten Schlüssel → Exit 2 vor dem ersten Crawl-Request (Request-Log leer).
  - [ ] `--email-on-complete a@b.test` ohne konfiguriertes E-Mail-Konto → Warnung auf stderr, Crawl läuft, Exit 0 (OUT-199).
  - [ ] `--set threads=abc` → Exit 1 mit Meldung „ungültiger Wert für threads: Ganzzahl 1–200 erwartet“; `--set unbekannt=1` → Exit 1 mit Vorschlag.
  - [ ] Läuft ohne Display (Linux-CI/SSH, Windows-Dienst); JS-Rendering funktioniert headless (`crawl-js/csr.html` liefert Titel aus dem Skript).
  - [ ] Unbekannte Option → Exit 1 mit Vorschlag der nächstliegenden Option (Levenshtein ≤ 3, z. B. `--crawl-lst` → „meinten Sie `--crawl-list`?“).
  - [ ] `--help` listet alle Flags gruppiert (Quelle, Konfiguration, Vergleich, APIs, Exporte, OS-Erweiterungen); OS-Erweiterungen sind in der Hilfe als solche markiert.
- **Testfixture:** `crawl-site-100`, `crawl-sitemaps`, `crawl-list-input/dupes.txt`, `crawl-js`; Integrationstests in `os-cli`.

### CRAWL-136 Geplante Crawls
- **Priorität:** P2
- **Beschreibung:** Wiederkehrende Crawls zu festen Zeiten (einmalig, täglich, wöchentlich, monatlich) mit Profil, Auth-Profil, API-Nutzung, Exporten und Benachrichtigungen. Die Ausführung erfolgt über den Aufgabenplaner des Betriebssystems (launchd / Windows-Aufgabenplanung), der das CLI aufruft. Export- und E-Mail-Details liegen im Export-Bereich.
- **Eingaben/Konfiguration:** Datei › Planung: Aufgabenliste mit „Hinzufügen“, „Bearbeiten“, „Löschen“, „Duplizieren“, „Verlauf“, „Export/Import“. Der Aufgabendialog folgt der Reiterstruktur der Referenz, damit Umsteiger Felder an gewohnter Stelle finden; verbindliche Feldspezifikation in OUT-160 bis OUT-171:
  - **Allgemein** (OUT-161): Aufgabenname, Projektname, Beschreibung, Datum/Uhrzeit, Intervall (Einmalig / Täglich / Wöchentlich / Monatlich).
  - **Spider** (OUT-162/163): Modus (Spider / Liste), Start-URL bzw. Listenquelle (Datei, XML-Sitemap-URL(s), Google-Tabelle), Konfigurationsprofil, Auth-Profil (CRAWL-121), „Automatisch vergleichen“ (CRAWL-036). Headless ist in OpenSpider immer aktiv (OUT-163); die Referenz-Option „Headless“ entfällt als Schalter.
  - **APIs** (OUT-164): Auswahl der Integrationen (GA4, Search Console, PageSpeed Insights, Majestic, Ahrefs, Moz).
  - **Exporte** (OUT-165) mit Unterreitern **Ausgabe-Einstellungen** (lokaler Ordner / Google Drive, Überschreiben vs. Zeitstempel-Ordner, Format, Konsolidierung, Segment), **Tabellen** (Tabs/Filter, Bulk-Exporte, Berichte), **Lokale Dateien** (Crawl-Datei, XML-Sitemap, Bild-Sitemap), **Looker Studio** (OUT-210), **Vorlagen/Presets** (gespeicherte Multi-Export-Auswahlen), **Benachrichtigungen** (E-Mail an Adressen, OUT-281).
  - Schalter **„Leere Berichte überspringen“** (Referenz 24.0; Verhalten OUT-012): Exporte ohne Datenzeilen werden weder geschrieben noch an E-Mails angehängt.
- **Steuerung laufender geplanter Crawls:** In der Referenz können laufende geplante Crawls weder pausiert noch abgebrochen werden. OpenSpider: **Pausieren** ist nicht möglich (Parität; ein geplanter Lauf ist ein Prozess ohne UI); **Abbrechen** ist als OS-Erweiterung möglich (OUT-171, Status „Abgebrochen“, Crawl wird gespeichert, keine Exporte).
- **Akzeptanzkriterien:**
  - [ ] Eine geplante Aufgabe mit Intervall „täglich 03:00“ legt einen Betriebssystem-Eintrag an, der das CLI mit den gespeicherten Argumenten aufruft (Test: generierte launchd-plist / Task-XML gegen Golden-Datei `daily-0300.plist` / `daily-0300.xml`).
  - [ ] Der Aufgabendialog zeigt genau die Reiter Allgemein, Spider, APIs, Exporte; der Reiter Exporte genau die sechs Unterreiter in der oben genannten Reihenfolge.
  - [ ] „Leere Berichte überspringen“ an: Ein Lauf mit Export „Antwortcodes › Clientfehler (4xx)“ gegen eine Fixture ohne 4xx erzeugt keine Datei für diesen Export; aus: eine Datei nur mit Kopfzeile.
  - [ ] Überlappende Ausführungen laufen als getrennte Prozesse und schreiben in getrennte Crawl-Dateien (zwei Aufgaben zur selben Minute → zwei `.ospider`-Dateien mit unterschiedlichen Datenbank-IDs).
  - [ ] Ein laufender geplanter Crawl bietet in der Aufgabenliste keine Aktion „Pausieren“; „Abbrechen“ beendet den Prozess innerhalb von 10 s (OUT-171).
  - [ ] Läuft auch im Akkubetrieb und ohne Zeitlimit (Referenz 18.2): generierte Windows-Task-XML enthält `DisallowStartIfOnBatteries=false`, `StopIfGoingOnBatteries=false`, `ExecutionTimeLimit=PT0S`.
  - [ ] Verlauf zeigt Listenmodus-Crawls ebenso wie Spider-Crawls (Referenz-Fix 21.4) mit Startzeit, Ende, Exit-Code, Anzahl URLs, Fehlertext.
  - [ ] Die Start-URL-Eingabe bietet zuletzt gecrawlte URLs an (Referenz 24.1; dieselbe Liste wie CRAWL-001).
- **Testfixture:** Golden-Dateien unter `tests/fixtures/crawl-schedule/`; `crawl-site-100` (ohne 4xx) für den Leer-Bericht-Test.

### CRAWL-137 Oberflächensprache
- **Priorität:** P2
- **Beschreibung:** Die Oberfläche ist mehrsprachig; beim ersten Start wird die Systemsprache übernommen, später ist sie in den Einstellungen änderbar. Mindestens Deutsch und Englisch (P2: Spanisch, Französisch, Italienisch wie in der Referenz). Exporte verwenden stabile Spaltenschlüssel; die Spaltenüberschriften folgen der UI-Sprache (umschaltbar auf Englisch für Weiterverarbeitung).
- **Eingaben/Konfiguration:** Einstellungen › Sprache.
- **Akzeptanzkriterien:**
  - [ ] Systemsprache Deutsch → deutsche UI beim ersten Start; nicht unterstützte Sprache → Englisch.
  - [ ] Sprachwechsel wirkt ohne App-Neustart (OS-Festlegung).
  - [ ] SERP-Upload akzeptiert Kopfzeilen in beiden Sprachen (CRAWL-031).
  - [ ] Zahlen- und Datumsformate folgen der UI-Sprache; CSV-Exporte verwenden immer `.` als Dezimaltrenner (OS-Festlegung, siehe Q).
- **Testfixture:** UI-Test (Vitest) mit simuliertem `navigator.language`.

---

## O. Ergänzungen (Revision 2)

Die folgenden Einträge wurden nach dem Review angehängt. Sie gehören thematisch zu den Abschnitten H (Erweitert), E/N (Konfiguration, Persistenz) und B (Vergleich); die Nummern folgen dem Schema „neue IDs anhängen“.

### CRAWL-138 HTML-Validierung durchführen
- **Priorität:** P1
- **Beschreibung:** Crawl-Schalter, der festlegt, ob die Engine beim Parsen jeder internen HTML-Seite eine strukturelle Prüfung des Quelltextes mitlaufen lässt (Token-Ebene, keine W3C-Vollvalidierung; Prüflogik in TECH-280). Die Prüfung ist nicht nur Auswertung: Sie verlangt, dass die Engine beim Parsen Zusatzdaten erhebt (Position und Anzahl von `<html>`, `<head>`, `<body>`, unzulässige Elemente im Head, Elemente, die den Head vorzeitig beenden). Ohne den Schalter werden diese Daten nicht erhoben.
- **Eingaben/Konfiguration:** Spider › Erweitert › „HTML-Validierung durchführen“ (Schlüssel `advanced.html_validation`; OS-Default: an – in der Referenz-Doku ist kein Standard genannt, die Beschreibung legt ein Opt-in nahe; siehe Q32). Begründung des OS-Defaults: Die Filter „außerhalb von `<head>`“ sind für Titel/Meta/Canonical sicherheitsrelevant für die Indexierung; die Mehrkosten der Token-Zählung sind gering.
- **Ausgabe (Spalten/Filter/Export):** Bei aktivem Schalter: Filter TECH-282 bis TECH-288 im Tab „Validierung“ sowie die Filter „außerhalb von `<head>`“ in den Tabs Seitentitel, Meta-Beschreibung, Canonicals, Hreflang, Meta Robots (anderer Bereich); intern je Seite die Rohdaten aus TECH-280. Bei inaktivem Schalter: diese Filter zeigen „Prüfung deaktiviert – Spider › Erweitert › HTML-Validierung“ statt 0.
- **Akzeptanzkriterien:**
  - [ ] Schalter an: `/kein-head.html` erscheint im Filter „Fehlendes `<head>`“; `/canonical-im-body.html` im Canonical-Filter „außerhalb von `<head>`“; `/ok.html` in keinem Validierungsfilter.
  - [ ] Schalter aus: dieselben Filter zeigen den Hinweis „Prüfung deaktiviert“ (nicht „0“); in der Crawl-Datei sind keine Validierungs-Rohdaten gespeichert (Tabellenzeilen = 0).
  - [ ] Die Größenfilter TECH-289/290 (Dokument/Ressource über Grenzwert) und die CO2-Schätzung TECH-291 hängen **nicht** von diesem Schalter ab (OS-Festlegung, Q32), weil sie nur Größenangaben aus CRAWL-022 benötigen.
  - [ ] Die Prüfung läuft immer gegen das Roh-HTML, auch im JS-Modus (TECH-280).
  - [ ] Schalter während einer Pause eingeschaltet: Seiten, die nach dem Fortsetzen gecrawlt werden, haben Validierungsdaten, vorher gecrawlte nicht; die Filter tragen dann den Hinweis „unvollständig – n Seiten ohne Prüfung, Neu crawlen empfohlen“.
  - [ ] Mehrkosten: `crawl-site-100` im Textmodus ist mit Schalter höchstens 5 % langsamer als ohne (Benchmark).
- **Testfixture:** `crawl-validation`: `/ok.html`, `/kein-head.html`, `/zwei-heads.html`, `/canonical-im-body.html` (ein `<img>` im Head beendet den Head vorzeitig, danach folgt das Canonical), `/title-nach-div.html`.

### CRAWL-139 Green Hosting für die CO2-Berechnung
- **Priorität:** P2
- **Beschreibung:** Crawl-Schalter (Referenz seit 20.0, Spider › Erweitert), mit dem der Nutzer angibt, dass die gecrawlte Website mit erneuerbarer Energie betrieben wird. Die CO2-Schätzung je Seite (TECH-291) verwendet dann den im verwendeten Sustainable-Web-Design-Modell vorgesehenen reduzierten Faktor für das Rechenzentrumssegment. OpenSpider prüft den Hosting-Status **nicht** automatisch bei einem Drittanbieter (kein Netzwerkzugriff ohne Nutzeraktion); der Dialog bietet lediglich einen Link „Hosting-Status prüfen“, der eine öffentliche Prüfseite im Systembrowser öffnet.
- **Eingaben/Konfiguration:** Spider › Erweitert › „Website nutzt grünes Hosting (für CO2-Berechnung)“ (Schlüssel `advanced.green_hosting`; OS-Default: aus – Referenz nennt keinen Standard, die Beschreibung legt „nur aktivieren, wenn zutreffend“ nahe; Q33). Wirkt nur auf interne URLs; externe Ressourcen werden stets mit dem Standardfaktor gerechnet (OS-Festlegung).
- **Ausgabe (Spalten/Filter/Export):** Spalten `CO2 (mg)` und `CO2-Bewertung` (Definition und Filter „Hohe CO2-Bewertung“ in TECH-291); Crawl-Übersicht nennt „Grünes Hosting: ja/nein“.
- **Akzeptanzkriterien:**
  - [ ] Schalter aus: `CO2 (mg)` der Fixture-Seite `/heavy.html` entspricht der Referenzimplementierung des Modells (CO2.js, Funktion für Byte-basierte Schätzung, `green = false`) für den Wert `Gesamt übertragen` (CRAWL-022) mit Toleranz 0,1 %.
  - [ ] Schalter an: Wert entspricht derselben Funktion mit `green = true` (Toleranz 0,1 %) und ist strikt kleiner als bei „aus“.
  - [ ] Umschalten nach dem Crawl berechnet `CO2 (mg)` und `CO2-Bewertung` ohne Neucrawl neu (Request-Zähler unverändert), Dauer < 2 s bei 10 000 URLs.
  - [ ] Externe Ressourcen, die eine interne Seite lädt, gehen bei Schalter „an“ mit dem Standardfaktor in die Summe ein (Test: Seite mit 1 MB internem und 1 MB externem Bild ergibt einen Wert zwischen „alles grün“ und „alles grau“).
  - [ ] Ohne Nutzeraktion wird kein Request an einen Green-Hosting-Prüfdienst gesendet (Netzwerkmitschnitt).
- **Testfixture:** `crawl-validation/heavy.html` (lädt dynamisch erzeugtes 6-MiB-Bild), `/mixed-host.html` (1 MiB intern + 1 MiB von `cdn.other.test`), JS-Modus.

### CRAWL-140 Voreinstellungen (crawl-relevante Grenzwerte)
- **Priorität:** P0 (Seite und Mechanik) / P1 (Größengrenzwerte)
- **Beschreibung:** Die Referenz bündelt Grenzwerte, die Filter steuern, unter Spider › Voreinstellungen. OpenSpider übernimmt diese Seite im Konfigurationsdialog. Die fachliche Wirkung jedes Grenzwerts ist im Zielbereich spezifiziert; dieser Eintrag legt fest, welche Grenzwerte die Seite enthält, wo sie verbindlich definiert sind und welche davon die **Crawl-Engine** betreffen (weil Rohdaten erhoben werden müssen). Grundregel: Filter werden aus gespeicherten Rohwerten berechnet, Grenzwertänderungen wirken daher ohne Neucrawl (ONPAGE-002).
- **Eingaben/Konfiguration:** Spider › Voreinstellungen, gruppiert:
  - **Seitentitel / Meta-Beschreibung / URL / H1 / H2 / Bilder (Größe, Alt-Text) / Inhalt (geringe Wortzahl)**: Schlüssel und Defaults verbindlich in ONPAGE 0.3 (u. a. `image.max_kb` = 100, `content.low_word_count` = 200).
  - **Soft-404-Text** (Referenz seit 19.0): Liste von Phrasen, `content.soft404_phrases`, verbindlich in ONPAGE-147.
  - **Links**: `high_internal_outlinks` (1 000, TECH-138), `high_external_outlinks` (100, TECH-137), `high_crawl_depth` (4, TECH-132), nicht beschreibende Ankertexte (TECH-Links).
  - **Größengrenzen für Suchmaschinen-Abruf** (Referenz: konfigurierbar, Version laut Review 23.3, siehe Q34): `validation.html_doc_max_bytes` (Standard 2 097 152 = 2 MiB, TECH-289) und `validation.resource_max_bytes` (Standard 2 097 152, TECH-290).
- **Ausgabe (Spalten/Filter/Export):** Filterbezeichnungen enthalten den aktuellen Wert (z. B. „HTML-Dokument über 2 MB“, „Bilder über 100 KB“).
- **Akzeptanzkriterien:**
  - [ ] Die Seite „Voreinstellungen“ zeigt alle oben genannten Gruppen; jeder Wert ist mit seinem Schlüssel im Profil gespeichert (Round-Trip CRAWL-132).
  - [ ] Crawl-seitige Pflicht: Für jede HTML-Seite und jede Ressource wird `Größe` (dekomprimiert, CRAWL-022) auch dann bestimmt, wenn sie den Grenzwert übersteigt; ist der Body wegen CRAWL-075 abgeschnitten, wird `Content-Length` bzw. bei fehlendem Header die bis zum Abbruch gelesene Größe mit Vermerk `Abgeschnitten` verwendet.
  - [ ] Ein HTML-Dokument mit 2 097 153 Bytes erscheint bei Standard im Filter TECH-289; nach Änderung von `validation.html_doc_max_bytes` auf 3 MiB nicht mehr – ohne neuen Request (Request-Zähler unverändert).
  - [ ] `validation.html_doc_max_bytes` größer als „Max. Seitengröße“ (CRAWL-075) wird beim Speichern mit Warnung „Grenzwert kann nicht erkannt werden, wenn Seiten vorher abgeschnitten werden“ angenommen, nicht abgelehnt.
  - [ ] Soft-404-Text: Eine leere Phrasenliste deaktiviert den Filter (Hinweis statt 0), die Engine erhebt aber weiterhin Titel/H1/Inhaltsbereich.
  - [ ] Ungültige Werte (negativ, min > max, nicht numerisch) werden beim Speichern abgewiesen, das Feld wird markiert.
  - [ ] „Standard zurücksetzen“ auf der Seite setzt nur die Voreinstellungen zurück, nicht die übrige Konfiguration.
- **Testfixture:** `crawl-validation/big-2mb-plus.html` (2 097 153 Bytes, gzip), `/big-2mb.html` (2 097 152 Bytes), `/big.js` (2,5 MiB); `onpage-content/soft404-*.html`.

### CRAWL-141 Automatische Crawl-Analyse am Crawl-Ende
- **Priorität:** P0
- **Beschreibung:** Einige Kennzahlen und Filter (u. a. Link Score, verwaiste URLs, Sitemap-Filter, Kanonisierungsketten, Near-Duplicates) benötigen den vollständigen Linkgraphen und werden in einem Nachlauf berechnet (Mechanik UI-080). Dieser Eintrag spezifiziert den Konfigurationsschalter und die crawl-seitige Auslösung. Mehrere Kriterien dieses Dokuments (CRAWL-027, CRAWL-059, CRAWL-131, CRAWL-134) setzen die Analyse voraus.
- **Eingaben/Konfiguration:** Konfiguration › Crawl-Analyse › „Automatisch am Crawl-Ende analysieren“ (Schlüssel `analysis.auto_at_end`; OS-Default: an – die Referenz beschreibt den Schalter, nennt aber keinen Standard, Q35; Festlegung konsistent mit UI-080). Dazu die Teilschritt-Auswahl (UI-080). Gehört zum Konfigurationsprofil.
- **Ausgabe:** Status „Analyse läuft“ mit eigenem Fortschritt (UI-080), danach „Abgeschlossen“; Crawl-Übersicht vermerkt Zeitpunkt der letzten Analyse.
- **Akzeptanzkriterien:**
  - [ ] Schalter an: Endet ein Crawl regulär (Warteschlange leer) oder durch ein Limit (CRAWL-066 ff.), startet die Analyse innerhalb von 1 s automatisch; Filter „Verwaiste URLs“ der Fixture ist danach befüllt.
  - [ ] Schalter an: Wird der Crawl pausiert, startet **keine** Analyse; erst nach Fortsetzen und regulärem Ende.
  - [ ] Schalter an, Abbruch durch Absturz: beim Wiederöffnen wird die Analyse nicht automatisch gestartet; die Filter zeigen „Crawl-Analyse erforderlich“.
  - [ ] Schalter aus: Nach Crawl-Ende läuft keine Analyse; analyseabhängige Filter zeigen „Crawl-Analyse erforderlich (Jetzt starten)“, nicht 0; „Crawl-Analyse › Starten“ führt sie manuell aus.
  - [ ] CLI/geplante Crawls: Der Schalter gilt auch headless; ist er aus und ein angeforderter Export enthält analyseabhängige Filter, wird die Analyse vor dem Export trotzdem ausgeführt und eine Hinweiszeile auf stderr ausgegeben (OS-Festlegung: Exporte sollen nie stillschweigend leer sein).
  - [ ] Listen-Crawl aus einer Sitemap (CRAWL-027): Sitemap-Filter sind erst nach der Analyse befüllt; mit Schalter an ist das ohne Nutzeraktion der Fall.
  - [ ] Wird während einer laufenden Analyse ein neuer Crawl gestartet, wird die Analyse sauber abgebrochen und der alte Crawl als „Analyse unvollständig“ markiert.
- **Testfixture:** `crawl-sitemaps` (verwaiste URL `/nur-in-sitemap`), Sitemap-Listen-Crawl, `crawl-site-100` mit Pause.

### CRAWL-142 Update-Prüfung und Fokus-Modus (Abgrenzung)
- **Priorität:** P2
- **Beschreibung:** Zwei Einstellungen der Referenz ohne Crawl-Logik, die hier nur zur Vollständigkeit verankert werden: (a) die automatische Prüfung auf neue Versionen (Referenz: Hilfe › Debug bzw. Konfigurationsdatei-Eintrag), (b) der Fokus-Modus der Oberfläche (Ausblenden ungenutzter Tabs). Verbindlich spezifiziert in **OUT-250** (Update-Prüfung, Einstellungen › „Automatisch nach Updates suchen“, Standard an, `updates.check` in `settings.toml`) bzw. **UI-005** (Fokus-Modus, Menü Ansicht). Dieser Eintrag legt nur die Wechselwirkungen mit der Crawl-Engine fest.
- **Eingaben/Konfiguration:** siehe OUT-250, UI-005; beide gehören zu den App-Einstellungen, nicht zum Crawl-Profil (CRAWL-132).
- **Akzeptanzkriterien:**
  - [ ] Eine Update-Prüfung oder ein Update-Download beeinflusst einen laufenden Crawl nicht: kein Neustart, keine Pause, keine Rate-Änderung > 5 % (Messung während simuliertem Download).
  - [ ] Ein Update wird nie installiert, solange ein Crawl läuft oder pausiert-ungespeichert ist; der Installationsdialog bietet „Nach Crawl-Ende installieren“.
  - [ ] Das CLI prüft nie automatisch auf Updates (OUT-250); ein geplanter Crawl erzeugt keinen Request an den Update-Server (Netzwerkmitschnitt).
  - [ ] Der Fokus-Modus ändert weder Konfigurationsprofil noch erhobene Daten: Ein Crawl mit und ohne Fokus-Modus ergibt byte-identische Crawl-Dateien (abgesehen von Zeitstempeln).
  - [ ] Weder Update-Prüfung noch Fokus-Modus erscheinen im Konfigurations-Diff (CRAWL-133).
- **Testfixture:** `crawl-site-100`; Update-Manifest-Mock.

### CRAWL-143 Kontextmenü der Crawl-Verwaltung
- **Priorität:** P1
- **Beschreibung:** Aktionen auf markierten Einträgen der Crawl-Verwaltung (CRAWL-128). Die Datenbank-ID ist die Brücke zur Kommandozeile (`--load-crawl <ID>`, `--crawl-comparison <ID> <ID>`) und muss daher direkt kopierbar sein. Der Export einzelner Crawls aus dem Kontextmenü muss auch für Crawls in Projektordnern funktionieren (Referenz-Fix 21.2).
- **Eingaben/Konfiguration:** Kontextmenü (Rechtsklick) und gleichnamige Schaltflächen: **Öffnen**, **Umbenennen**, **In Projekt verschieben…**, **Duplizieren**, **Crawl exportieren…** (speichert eine `.ospider`-Kopie an frei wählbarem Ort), **Datenbank-ID kopieren**, **Sperren** / **Entsperren**, **Löschen**, **Zum Vergleich auswählen** (nur bei genau zwei markierten Crawls), **Im Dateimanager zeigen** (OS-Erweiterung). Für Projektordner: **Projekt umbenennen**, **Projekt sperren/entsperren**, **Projekt exportieren** (alle Crawls des Projekts als ZIP, OS-Erweiterung).
- **Akzeptanzkriterien:**
  - [ ] „Datenbank-ID kopieren“ legt genau die ID (ohne Leerzeichen, ohne Zeilenumbruch) in die Zwischenablage; bei Mehrfachauswahl eine ID je Zeile in Tabellenreihenfolge.
  - [ ] `openspider --load-crawl <eingefügte ID> --export-tabs "Intern:Alle"` exportiert genau diesen Crawl (End-to-End-Test).
  - [ ] „Crawl exportieren…“ funktioniert für einen Crawl ohne Projekt **und** für einen Crawl in einem Projektordner; die exportierte Datei lässt sich auf einem anderen Rechner importieren und ergibt identische Tabelleninhalte (CRAWL-127).
  - [ ] Export eines laufenden Crawls ist deaktiviert (Tooltip „Crawl läuft – erst pausieren“); Export eines pausierten Crawls ist möglich und der Crawl bleibt fortsetzbar.
  - [ ] „Zum Vergleich auswählen“ ist bei 1 oder ≥ 3 markierten Einträgen ausgegraut; bei zwei Einträgen öffnet es den Vergleichsmodus mit dem jüngeren als „Aktuell“ (CRAWL-032).
  - [ ] „Löschen“ und „Umbenennen“ sind für gesperrte Crawls ausgegraut; „Entsperren“ verlangt keine Bestätigung, „Löschen“ immer.
  - [ ] Jede Aktion ist per Tastatur erreichbar (Kontextmenü-Taste bzw. Umschalt+F10), Löschen zusätzlich über Entf/Rücktaste mit Bestätigung.
- **Testfixture:** Integrationstest in `os-store` + UI-Test: 3 Crawls (einer im Projekt „Kunde A“, einer gesperrt, einer pausiert).

### CRAWL-144 Vergleichsmodus: Detailansichten (Links, Issues, gerenderte Seite)
- **Priorität:** P1 (Inlinks/Outlinks, Issues) / P2 (gerenderte Seite)
- **Beschreibung:** Ergänzung zu CRAWL-032: Im Vergleichsmodus zeigt das untere Fenster die Daten der markierten URL aus beiden Crawls, und die Issues-Übersicht stellt Veränderungen je Issue dar.
- **Eingaben/Konfiguration:**
  - **Unteres Fenster, Reiter Inlinks/Outlinks:** Auswahlfeld „Quelle“ mit **Auto** (Standard; folgt der Auswahl Aktuell/Vorher des oberen Fensters), **Aktueller Crawl**, **Vorheriger Crawl**; zusätzliches Auswahlfeld „Änderung“ mit **Alle**, **Hinzugefügt**, **Entfernt**, **Unverändert**.
  - **Issues-Ansicht** (rechte Seitenleiste bzw. Issues-Tab, UI-Bereich) im Vergleich.
  - **Unteres Fenster, Reiter „Gerenderte Seite“:** Nebeneinander-Darstellung der Screenshots beider Crawls (CRAWL-082).
- **Definitionen:** Für eine URL u, die in beiden Crawls vorkommt, sei L_C(u) bzw. L_P(u) die Menge ihrer Links im jeweiligen Crawl, wobei ein Link durch das Paar (Ziel-URL nach URL-Zuordnung CRAWL-034, Linktyp) identifiziert wird. Hinzugefügt = L_C(u) \ L_P(u); Entfernt = L_P(u) \ L_C(u); Unverändert = L_C(u) ∩ L_P(u). Änderungen nur des Ankertexts oder der Position zählen als „Unverändert“ (OS-Festlegung; Ankertext-Änderungen sind über die Änderungserkennung CRAWL-033 sichtbar). Für Inlinks gilt dasselbe mit den eingehenden Links.
- **Ausgabe (Spalten/Filter/Export):** Inlinks/Outlinks-Tabelle mit den Spalten aus CRAWL-052 plus `Änderung` (Hinzugefügt/Entfernt/Unverändert) und `Crawl` (Aktuell/Vorher); Issues-Ansicht mit Spalten `Issue`, `Typ`, `Priorität`, `Aktuell`, `Vorher`, `Änderung`, `Hinzugefügt`, `Neu`, `Entfernt`, `Fehlend` (Mengen wie CRAWL-032, Filter = „URL hat dieses Issue“); gerenderte Seite mit Beschriftung je Bild: Crawl-Name, Datum, User-Agent, Viewport.
- **Akzeptanzkriterien:**
  - [ ] Quelle „Auto“: Steht das obere Fenster auf „Vorher“, zeigt der Inlinks-Reiter die Inlinks aus dem vorherigen Crawl; Umschalten oben auf „Aktuell“ wechselt den Reiter ohne weitere Aktion.
  - [ ] Fixture v1→v2: Seite `/start` verliert in v2 den Link auf `/b` und erhält einen auf `/a`. Outlinks von `/start` mit Filter „Hinzugefügt“ = {`/a`}, „Entfernt“ = {`/b`}; mit „Alle“ und Quelle „Aktueller Crawl“ fehlt `/b`.
  - [ ] Nur geänderter Ankertext (`/c` „Alt“ → „Neu“) erscheint unter „Unverändert“, nicht unter „Hinzugefügt“/„Entfernt“.
  - [ ] Ist u nur in einem Crawl vorhanden, sind die Filter „Hinzugefügt“/„Entfernt“ ausgegraut und die Tabelle zeigt die Links des Crawls, in dem u existiert.
  - [ ] Issues-Ansicht: Für das Issue „Titel fehlt“ gelten dieselben Zahlen wie im Tab-Vergleich von CRAWL-032 (`/c` Hinzugefügt, `/d` Entfernt); die Identität `Aktuell` − `Vorher` = |Hinzugefügt| + |Neu| − |Entfernt| − |Fehlend| gilt für jedes Issue.
  - [ ] Issues-Ansicht ist nach `Änderung` sortierbar; Klick auf eine Zahl listet die URLs im oberen Fenster.
  - [ ] Gerenderte Seite: Sind in beiden Crawls Screenshots vorhanden, werden beide nebeneinander in gleicher Skalierung gezeigt; typischer Anwendungsfall „Desktop- gegen Mobil-Crawl“ (Crawl 1 mit Googlebot Desktop, Crawl 2 mit Googlebot Mobil, CRAWL-081/114) zeigt unter jedem Bild den jeweiligen User-Agent und Viewport.
  - [ ] Fehlt in einem Crawl der Screenshot (Textmodus oder Screenshots aus), zeigt die betreffende Hälfte „Kein Screenshot in diesem Crawl“.
  - [ ] Export der Inlinks/Outlinks-Ansicht im Vergleich enthält die Spalten `Änderung` und `Crawl` (Verweis OUT-151).
- **Testfixture:** `crawl-compare` (v1/v2) mit zusätzlicher Seite `/start` (Linkänderungen wie oben, `/c` mit geändertem Ankertext); zwei JS-Crawls derselben Fixture mit Googlebot Desktop bzw. Googlebot Mobil für die Screenshot-Gegenüberstellung.

---

## Q. Offene Punkte / zu verifizieren

Diese Punkte sind in der öffentlichen Doku nicht eindeutig. Sie werden per Black-Box-Vergleich gegen die Referenz (normale Bedienung, eigene Fixture, ≤ 500 URLs) geprüft; Ergebnis wird hier nachgetragen.

| # | Funktion | Frage | Vorläufige OS-Festlegung |
|---|---|---|---|
| Q1 | CRAWL-003 | Werden `/x?` und `/x` als eine URL behandelt? | getrennt |
| Q2 | CRAWL-006 | Werden externe HTML-Seiten geparst (Titel etc. erfasst)? | nein |
| Q3 | CRAWL-006 | Gilt ein anderer Port auf demselben Host als intern? | intern |
| Q4 | CRAWL-009 | Unterordner + Alle Subdomains: Ordnergrenze je Subdomain? | ja |
| Q5 | CRAWL-014 | Wird der Body einer 3xx-Antwort nach Links durchsucht? | nein |
| Q6 | CRAWL-028 | Werden im Listenmodus (Tiefe 0) Ressourcen der Seiten geprüft? | nein |
| Q7 | CRAWL-039 | Werden `data-src`/Lazy-Attribute im Textmodus als Bilder erkannt? | nein |
| Q8 | CRAWL-042 | Standardwerte für Medien speichern/crawlen? | an/an |
| Q9 | CRAWL-052 | Wird `<noscript>` im Textmodus nach Links geparst? | ja |
| Q10 | CRAWL-055 | Standard für „Bilder aus srcset extrahieren“? | aus |
| Q11 | CRAWL-060 | Akzeptiert der Sitemap-Crawl Text-Sitemaps und RSS/Atom? | ja |
| Q12 | CRAWL-061 | Standardwerte der Seitendetail-Schalter? | alle an außer Barrierefreiheit |
| Q13 | CRAWL-067 | Werden Ressourcen der Seiten auf maximaler Tiefe geprüft? | ja |
| Q14 | CRAWL-072 | Standardwert „Max. Weiterleitungen“? | 10 |
| Q15 | CRAWL-074 | Zählen Ressourcen-Links zum Limit „Max. Links pro URL“? | ja |
| Q16 | CRAWL-089 | Standard „Nicht indexierbare URLs ignorieren“? | an |
| Q17 | CRAWL-090 | Standard „Paginierte URLs ignorieren“? | an |
| Q18 | CRAWL-094 | Gilt ein Refresh ohne URL als selbstreferenzierend? | ja |
| Q19 | CRAWL-096 | Standardanzahl der 5xx-Wiederholungen? | 0 |
| Q20 | CRAWL-115 | Werden eigene Header auch an externe Hosts gesendet? | nein (Sicherheitsentscheidung, bewusste Abweichung möglich) |
| Q21 | CRAWL-127 | Import von Referenz-Crawldateien? | nicht vorgesehen (proprietär, Clean-Room) |
| Q22 | CRAWL-137 | Dezimaltrenner in CSV-Exporten bei deutscher UI? | immer `.` |
| Q23 | Allgemein | Regex-Dialekt erfordert `fancy-regex` statt `regex` in `os-url` – Architekturentscheidung durch Orchestrator. | `fancy-regex` |
| Q24 | CRAWL-119 | NTLM/Kerberos: in Referenz-Doku nicht erwähnt – Bedarf bestätigen. | P2 |
| Q25 | CRAWL-109/110/076 | Abgleichsemantik Include/Exclude: Doku verlangt für Include ausdrücklich Vollübereinstimmung; Exclude-Beispiele sind in verkürzter Wiedergabe als Teiltreffer lesbar. Black-Box: Exclude `\.pdf` (ohne `.*`) gegen `/x.pdf` – wird ausgeschlossen? | Vollübereinstimmung für beide; Schalter für Teiltreffer |
| Q26 | CRAWL-018 | Standardwert „HSTS respektieren“ (Doku nennt nur Abschaltbarkeit) | an (abgeleitet) |
| Q27 | CRAWL-027 | Erkennt der Listen-Upload URLs ohne Schema (`www.site.test/a`)? | nein, als ignorierte Zeile gezählt |
| Q28 | CRAWL-047 | Standard „Paginierung speichern“ (Doku: nur „nicht gecrawlt, nicht zur Entdeckung genutzt“) | Speichern an, Crawlen aus |
| Q29 | CRAWL-062/065 | Standardwerte der URL-Details (Antwortzeit, Last-Modified, HTTP-Header, Cookies) und Direktiven-Schalter | an/an/aus/aus; Meta Robots + X-Robots-Tag an |
| Q30 | CRAWL-100 | Verhalten der Referenz bei robots.txt 5xx/429/Timeout/DNS-Fehler (Doku schweigt; Risiko: Referenz crawlt weiter) | Host gesperrt (konfigurierbar „Alles erlauben“) |
| Q31 | CRAWL-135 | Exakte Syntax von `--crawl-google-sheet` (nur Release-Notes/Sekundärquellen gefunden: `<Konto> <Tabellen-URL>`) | `--crawl-google-sheet <Konto> <Tabellen-URL>` |
| Q32 | CRAWL-138 | Standard „HTML-Validierung durchführen“; hängen Größenfilter (2 MB) und CO2 von diesem Schalter ab? | an; Größenfilter/CO2 unabhängig |
| Q33 | CRAWL-139 | Standard „Green Hosting“; gilt der Faktor auch für externe Ressourcen? | aus; nur interne URLs |
| Q34 | CRAWL-140 | Seit welcher Version sind die 2-MB-Grenzen (Dokument/Ressource) einstellbar, und heißen sie in den Voreinstellungen genau so? (Review nennt 23.3; TECH-289 nennt 24.3 als Stand mit 2 MB, vorher 15 MB) | einstellbar, Standard 2 MiB |
| Q35 | CRAWL-141 | Standard „Automatisch am Crawl-Ende analysieren“ | an |
| Q36 | CRAWL-089 / ONPAGE 0.3 | Widerspruch Standard `issues.ignore_non_indexable`: 01 = an, 02 (A-51) = aus | Orchestrator entscheidet; bis dahin Wert aus 02 |
| Q37 | CRAWL-144 | Wie identifiziert die Referenz „hinzugefügte/entfernte“ Links (nur Ziel, oder Ziel + Ankertext)? | Ziel + Linktyp, Ankertext ignoriert |

---

## R. Quellen

Öffentliche Seiten des Referenzherstellers (abgerufen am 2026-10-13), ausschließlich als Verhaltensbeschreibung genutzt:
- Benutzerhandbuch, Konfiguration: https://www.screamingfrog.co.uk/seo-spider/user-guide/configuration/
- Benutzerhandbuch, Allgemein (Modi, Speichern, Planung, CLI, Export): https://www.screamingfrog.co.uk/seo-spider/user-guide/general/
- Benutzerhandbuch, Tabs (Antwortcodes, Intern, URL): https://www.screamingfrog.co.uk/seo-spider/user-guide/tabs/
- Tutorial Crawl-Vergleich: https://www.screamingfrog.co.uk/seo-spider/tutorials/how-to-compare-crawls/
- Benutzerhandbuch, Kommandozeile (Abschnitt in „Allgemein“) – Flag-Syntax `--crawl-list`, `--project-crawl-comparison`, `--use-*`, `--email-on-complete`: https://www.screamingfrog.co.uk/seo-spider/user-guide/general/
- Benutzerhandbuch, Konfiguration – Abschnitte Erweitert (HTML-Validierung, Green Hosting), Voreinstellungen, Crawl-Analyse, Proxy, Include/Exclude, URL-Umschreibung: https://www.screamingfrog.co.uk/seo-spider/user-guide/configuration/
- Sekundärquelle (nur für Q31, nicht normativ): Community-Übersicht zu CLI-Flags, https://chrisleverseo.com/forum/t/screaming-frog-seo-spider-cli.150/
- Tutorial JavaScript-Crawling: https://www.screamingfrog.co.uk/seo-spider/tutorials/crawl-javascript-seo/
- Leitfaden Authentifizierung: https://www.screamingfrog.co.uk/guides/authentication
- Release-Historie: https://www.screamingfrog.co.uk/seo-spider/release-history/
- Release-Notes: https://www.screamingfrog.co.uk/seo-spider-18/, …-19/, …-20/, …-21/, …-22/, …-23/, …-24/

Normative Fremdstandards: RFC 9309 (robots.txt), RFC 6265 (Cookies), RFC 6797 (HSTS), RFC 7616 (Digest), RFC 9110 (HTTP-Semantik), WHATWG URL- und Encoding-Standard, sitemaps.org-Protokoll, Public Suffix List.
