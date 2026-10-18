# UI-Lastfixture (M2-K19 / UI-148)

Vorgesehener Aufruf aus der Repository-Wurzel nach der Binary-Verdrahtung:

```sh
cargo run --release --locked -p os-loadtest --bin gen-ui-large -- --output target/ui-large.ospider
cargo run --release --locked -p os-loadtest --bin gen-ui-large -- --output target/ui-small.ospider --rows 100000 --seed 19
```

Integration noch offen: Das Manifest registriert `src/bin/gen-ui-large.rs`
(F05-Stub mit Besitzervermerk M2-K05), während M2-K19 laut `owns` nur
`src/bin/gen_ui_large.rs` ändern darf. Generator und CLI-Argumente sind in
diesem Modul implementiert und getestet. Die Ersetzung des registrierten
Stubs benötigt eine ausdrückliche Erweiterung des Dateibesitzes; sie ist
keine additive Schnittstellenergänzung nach K2-61. Der bestehende Stub bleibt
bis dahin unverändert. Im Einstieg sind nur `Args::parse().run()`, die
JSON-Ausgabe des Berichts und die Behandlung des Fehler-Exitcodes zu ergänzen.

Standard: 5 000 000 URLs, 100 000 000 Hyperlinks, Seed 19. Unterstützt werden
22 bis 5 000 000 URLs. Das Minimum hält die 20 Ziele je Quelle verschieden,
ohne Selbstlinks. Die Ausgabe ist eine abgeschlossene `.ospider`-Datei im
aktuellen Store-Schema (v4, einschließlich der v3-Leseschnittstelle), keine
Simulation einer API. Der Generator benötigt weder Netzwerk noch Chromium.

Alle URLs sind intern und HTML; 99 % haben Status 200, 1 % Status 404.
Über 65 native URL-Spalten sowie die
Linkzähler stehen für Tabellenprojektionen bereit. Die Verteilungen enthalten
fehlende und leere Titel, Duplikate, Unicode, 300 Zeichen lange Titel,
verschiedene Wortzahlen einschließlich 0/2/10/NULL, variable Antwortzeiten,
Größen, Tiefen und noindex. Die Graphstruktur bleibt zwischen Seeds gleich;
Spaltenwerte, Crawl-ID und gespeicherter Seed ändern sich. Zeitstempel sind
fest. Wiederholungen mit identischen Optionen und derselben Programmversion
erzeugen dieselben Datei-Bytes; Laufzeitangaben erscheinen nur im JSON-Bericht.

Ausreißer:

- URL-ID 1 (`https://ui.test/`): genau min(N−1, 1 000 000) Inlinks von
  verschiedenen Quellen. Bei der Standardgröße sind das 1 Mio. Inlinks.
- URL-ID 2: genau 15 MiB vollständiges, unkomprimiertes HTML in `page_html`;
  über `os_store::queries::stored_html` lesbar. `size_bytes` stimmt damit überein.

Die Zähler entsprechen den gespeicherten Links. Nach dem Bulk-Laden werden
die Sekundärindizes aufgebaut, ausgenommen die fünf nach Store-Regel K-83 bei
großen Dateien entfallenden Textindizes. Ein SQL-Block verarbeitet höchstens
10 000 URLs und 200 000 Links; der SQLite-Cache ist auf 64 MiB eingestellt,
temporäre Sortierung läuft auf Disk. Für den großen Lauf sollte freier
Plattenplatz für Datenbank, WAL und temporäre Indexsortierung vorhanden sein.

Die Erzeugung erfolgt in einem privaten temporären Ordner am Zielort. Erst
nach Fertigstellung und WAL-Checkpoint wird die Datei ohne Überschreiben
veröffentlicht. Bestehende Ziele werden abgewiesen. Fortschritt geht nach
stderr, ein JSON-Bericht mit Größe, Zeilen, Links, Seed und Laufzeit nach stdout.

Prüfungen:

```sh
cargo test --locked -p os-loadtest --test gen_ui_large_fixture
cargo test --release --locked -p os-loadtest --test gen_ui_large_fixture five_million_rows -- --ignored --nocapture
```

Die 100 000-Zeilen-Fixture läuft regulär. Der große Test ist ausschließlich
ein Benchmark-Ignore und erzwingt die Zehn-Minuten-Grenze für die Erzeugung
einschließlich Indizes, Checkpoint und Veröffentlichung. Nightly-Job und
UI-Performancebudgets liegen bei M2-U31.

Lokaler Nachweis (macOS arm64, 16 GiB RAM, Release-Build): Der Benchmark-Test
erzeugte mit Seed 19 insgesamt 5 000 000 URLs und 100 000 000 Links in
309 321 ms einschließlich Indexaufbau, Checkpoint und Veröffentlichung.
Dateigröße: 16 184 958 976 Bytes. Die anschließenden Prüfungen bestätigten
genau 1 000 000 verschiedene Inlink-Quellen für URL 1, davon 990 000 interne
HTML-2xx-Quellen, und die Zähler an den Ring-/Hub-Grenzen. Das Zehn-Minuten-
Kriterium ist damit lokal erfüllt; dies ersetzt keinen UI-Benchmark in M2-U31.
