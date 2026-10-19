# UI-Lastfixture (M2-K19 / UI-148)

Aufruf aus der Repository-Wurzel:

```sh
cargo run --release --locked -p os-loadtest --bin gen-ui-large -- --output target/ui-large.ospider
cargo run --release --locked -p os-loadtest --bin gen-ui-large -- --output target/ui-small.ospider --rows 100000 --seed 19
```

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
Größen, Tiefen und noindex. Die Tiefe ist ein Verteilungswert, nicht die
Graph-Distanz (siehe Modul-Dokumentation); für Distanzprüfungen in UI-066
ist diese Fixture nicht vorgesehen. Die Graphstruktur bleibt zwischen Seeds gleich;
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

Übergabe an den nightly-Job von M2-U31 (UI-148): Nach dem Release-Build die
stdout-Ausgabe als `target/loadtest/gen-ui-large.json` aufbewahren. Beispiel
für den Erzeugungsschritt in `.github/workflows/ui-nightly.yml` (Besitzer U31):

```yaml
- name: UI-Fixture und Generatorbericht
  shell: bash
  run: |
    mkdir -p target/loadtest
    target/release/gen-ui-large --output target/ui-large.ospider > target/loadtest/gen-ui-large.json
    python3 -c 'import json; r=json.load(open("target/loadtest/gen-ui-large.json")); assert r["rows"] == 5000000 and r["elapsed_ms"] <= 600000, r'
- uses: actions/upload-artifact@v4
  if: always()
  with:
    name: gen-ui-large-${{ runner.os }}-${{ runner.arch }}
    path: target/loadtest/gen-ui-large.json
    if-no-files-found: error
```

Der Bericht enthält `rows`, `links`, `seed`, `hub_inlinks`, `html_bytes`,
`schema_version`, `elapsed_ms` und `file_bytes`. U31 historisiert die Berichte
je Plattform (macOS arm64 / Windows x64) und vergleicht die Laufzeit mit dem
Budget sowie dem gleitenden Median der letzten sieben Läufe (+10 %).
Die große Datenbank selbst wird nicht als CI-Artefakt benötigt.

Lokaler Nachweis (macOS arm64, 16 GiB RAM, Release-Build): Der Benchmark-Test
erzeugte mit Seed 19 insgesamt 5 000 000 URLs und 100 000 000 Links in
309 321 ms einschließlich Indexaufbau, Checkpoint und Veröffentlichung.
Dateigröße: 16 184 958 976 Bytes. Die anschließenden Prüfungen bestätigten
genau 1 000 000 verschiedene Inlink-Quellen für URL 1, davon 990 000 interne
HTML-2xx-Quellen, und die Zähler an den Ring-/Hub-Grenzen. Das Zehn-Minuten-
Kriterium ist damit lokal erfüllt; dies ersetzt keinen UI-Benchmark in M2-U31.
