# CI-Regression M2-E07

`cargo test --locked -p os-store --test scale_storage -- --nocapture` erfolgreich: 10k 6850,15 Bytes/URL, 50k 6930,60 Bytes/URL, Verhältnis 1,01174 (Grenze 1,3). Ungenutzter B-Tree-Raum einschließlich Freiliste: 10,9537 % / 10,4455 % (Grenze 15 %). Laufzeit des Tests auf dem lokalen Mac unter Fremdlast 47,29 s; keine Laufzeitgrenze im CI-Test.

Die Fixture schreibt 60k URLs insgesamt über den echten Writer, je 30 Links, 24 skalare Fakten und zwei 1100-Byte-Texte. 30 % Wachstumstoleranz lassen Varint-Breiten und weitere B-Tree-Ebenen zu; 15 % ungenutzter Raum erlauben normale Blattfüllung, schließen aber die vorherigen fast leeren Overflow-Seiten aus. Kein VACUUM oder manuelles Verdichten vor der Messung.

`.github/workflows/scale-nightly.yml`: täglich 02:30 UTC und workflow_dispatch, macos-15, Release-Build --locked, ruhige Starts, 100k/250k, JSON/Markdown und Vergleich als Artefakt (14 Tage). Job continue-on-error; keine blockierende Abnahme. YAML lokal geparst; tatsächlicher GitHub-Lauf nicht ausgelöst (kein Push/PR gemäß Auftrag).

Die Messhilfe prüft inzwischen zusätzlich den vollständigen Analysezustand (`completed`, keine `pending`-Schritte). Ein CLI-Erfolgscode allein reicht nicht. RSS-Spitzen werden zusätzlich über den Betriebssystemwert der beendeten Kinder erfasst; das CLI dominiert die kleinen Hilfsprozesse. Die Nacht-Auswertung verwendet den gesamten dbstat-ungenutzten Raum inklusive SQLite-Autoindizes und Freiliste sowie beide WAL-Phasengrenzen.

Die lokale Workspace-Prüfung kann bei knappem Plattenplatz mit `RUSTC_WRAPPER="$PWD/docs/plan/M2-acceptance/scale/rustc-test-wrapper.py"` ausgeführt werden (zusätzlich zu den C5-Profilvariablen). Der Wrapper ruft rustc unverändert auf und entfernt erst nach erfolgreichem Linken Symbole aus Testbinaries. `logging_cli` und `logging_rotation` behalten ihre Symbole, weil ihre unveränderten Assertions Funktionsnamen im Backtrace prüfen. Die Nachtmessung verwendet den Wrapper nicht.
