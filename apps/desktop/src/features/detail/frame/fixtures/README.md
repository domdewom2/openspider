# U11-Vertragsfixtures

`u11-contract.json` beschreibt ausschließlich die UI-/IPC-Verträge des Detailfensters,
abgeleitet aus `docs/spec/05-ui.md` und den dort referenzierten Fach-Specs.
`u11-depth-contract.json` enthält den Vertragsfall Tiefe 7 mit acht Pfadschritten
(UI-050/OUT-023). Beide Dateien werden von den eigenen Komponenten- und E2E-Tests gelesen.

Diese Dateien ersetzen keine gemeinsamen Spec-Fixtures. Insbesondere bleibt
`tests/fixtures/ui/fx-ui-basic/**` bei M2-K04; der echte Kern-/Bridge-Test ist nach C9
bis zur Integration von K04/K06 zurückgestellt.
