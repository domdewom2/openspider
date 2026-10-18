# Spec-Abdeckung

M1 bleibt mit den bisherigen Status- und Ignore-Regeln prüfbar:

```sh
cargo run --locked -p os-parity -- coverage --milestone M1 \
  --test-output target/rust-tests.txt --strict
```

Für M2 werden alle `spec_ids` der kanonischen Paketliste eingelesen, auch die der
Spikes und Messpakete. Ohne `--packages` wird die Liste unter `--root` gewählt.
Testergebnisse stammen aus echten Läufen, nicht aus den eingecheckten Statusdateien:

```sh
cargo test --workspace --exclude openspider-desktop --locked > target/rust-tests.txt 2>&1
npm run -w apps/desktop test -- --reporter=junit --outputFile.junit=../../target/vitest.xml
PLAYWRIGHT_JUNIT_OUTPUT_FILE=../../target/playwright.xml \
  npm run -w apps/desktop e2e -- --reporter=junit
cargo run --locked -p os-parity -- coverage --milestone M2 \
  --test-output target/rust-tests.txt \
  --frontend-junit target/vitest.xml --frontend-junit target/playwright.xml \
  --write docs/plan/M2-status.json
```

`--junit` nimmt weiterhin Nextest-JUnit an. `--frontend-junit` ist wiederholbar,
auch für mehrere Browser. Ein fehlgeschlagener Lauf überschreibt einen grünen
Beleg desselben Tests. Datei und vollständiger Testtitel müssen eindeutig sein.
Der statische Frontend-Scan unterstützt literale und Template-Titel von `test`/`it`
sowie `describe`/`test.describe`-Gruppen. Bei Templates müssen Dateipfad und
statische Titelteile eindeutig zu einer Quelle passen; alle ausgegebenen Fälle
werden zusammengeführt (ein roter Fall genügt). `.each` und vollständig berechnete
Titel erhalten keine Gutschrift; dafür einen direkten `test()`-/`it()`-Aufruf nutzen.
Strings, Templates, Blockkommentare und Regex-Literale erzeugen keine Tags.

Entwicklung wertet die Wellen 0/1 aus. Offene Pflicht-IDs späterer Wellen sowie
zulässige C9-Platzhalter werden als Warnungen ausgegeben. Die Platzhalter werden
an Paketbesitz, Ort, Quell- und Zielwelle geprüft und bleiben im Status `open`.
Gültige Zurückstellungen (`M3:`, `M4:`, `M5:`, `P2:`, `Klärung K2-<Zahl>`) sind
`deferred`; Benchmarks sind neutral. Ungültige Ignores, insbesondere `M2:`,
bleiben Fehler, auch ohne Spec-Tag. Ein tatsächlich roter Pflicht-Test bleibt
auch in einer späteren Welle ein Fehler.

`--gate` prüft alle Wellen, fordert jede Pflicht-ID als `implemented`/`verified`
und verbietet sämtliche Platzhalter. Diese Option ist für die Q04-Abnahme und
als manuelle Eingabe des CI-Workflows vorgesehen.

Anhang B wird aus `docs/plan/M2.md` gelesen und je `(ID, Paket)` für die dort
zugewiesenen `extra_spec_ids` berichtet. Die Belegdatei muss zum `owns`-Muster des
Pakets passen; jeder verlangte Marker (`[P1]`, `[M2]`) braucht einen grünen Test.
Marker verschiedener Kommentarzeilen werden nicht zwischen IDs übertragen.
Ausnahmen aus Abschnitt 9, einschließlich AMP (K2-22), zählen nicht.

Die deterministische Statusdatei enthält `specs`, `extra`, `appendix_b`,
`placeholders`, `blocking_specs` und `blocking_appendix_b`. Ein fehlender Testlauf
liefert keine Gutschriften. Frische Paritätsbelege können wie in M1 über
`--parity-report`/`--evidence` ergänzt werden; vorhandenes `verified` bleibt nur
bei weiterhin grünem Test und einer M2-Statusdatei erhalten.
