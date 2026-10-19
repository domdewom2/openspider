# M2-D03 Bild-Fixtures

Alle Inhalte sind eigens erzeugt. Keine Referenzdaten.

- `index.html`, `scenario.toml`: Loopback-Crawl mit srcset an img/picture, separaten Kandidaten und CSS. Der Request-Log beweist, dass srcset bei ausgeschalteter Option nicht angefragt wird.
- `parse/sources.html`: Pfadarten, XPath mit Geschwisterindizes, Linkpositionen, srcset-Deskriptoren und Komma in einer URL, ignorierte Metadaten/Lazy-Load-Attribute und Inline-CSS.
- `parse/header.png`: selbst erzeugtes, vollständiges PNG (800×600). Die übrigen Binärdateien sind kleine synthetische Metadaten-Fixtures, keine dekodierbaren Bildbeispiele. JPEG (progressiver SOF), GIF und drei WebP-Varianten melden ebenfalls 800×600. AVIF enthält zuerst ein Thumbnail (40×30) und verknüpft das primäre Item ausdrücklich mit der zweiten ispe-Eigenschaft (800×600).
- `analysis/facts.sql`: persistierte F05-Rendering-Verträge. Gemischte gute/leere Alt-Instanzen, fehlendes Alt, 101 Zeichen, exakt 100 KiB/100 KiB + 1 Byte, CSS/img über mehrere Seiten, dynamischer Hintergrund, Einsparung 4095/4096/4097 Byte, hochskaliertes/unsichtbares/SVG/Bild außerhalb des Viewports und ein ungecrawlter srcset-Kandidat. Rendering-Fakten werden hier injiziert; die Browser-Abnahme gehört M2-R02.
- `export/missing-alt-text.csv`: selbst geschriebener Erwartungswert für die betroffene Instanz; CSV und XLSX werden über die echte Writer-Pipeline geprüft.

Dateiformatgrundlagen: [PNG](https://www.w3.org/TR/png-3/), [WebP](https://developers.google.com/speed/webp/docs/riff_container), [AVIF](https://aomediacodec.github.io/av1-avif/index.html). Der Bildkopfleser dekodiert keine Pixel.

Paketbericht mit Prüfungen, Fable-Fixes und K2-63-Erwartungsänderung: [M2-D03](../../../docs/plan/M2-reports/d03.md).

Offene Übergabe an Planer/N01: [K2-Klärung zu ONPAGE-179](K2-clarifications.md).
