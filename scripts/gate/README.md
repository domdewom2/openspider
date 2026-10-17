# Aufrufhilfen für die lokale Gate-Gegenprobe (M1-N26, genutzt in M1-N28)

Nur Aufrufhilfen, kein Produktcode. Reihenfolge nach den Messblöcken B1 bis B4 (`scripts/bench/run_acceptance.sh`):

| Schritt | Aufruf |
|---|---|
| 1 Workspace-Tests (frisches Target, ohne `RUSTC_WRAPPER`) | `scripts/gate/workspace_tests.sh <ausgabe-ordner>` |
| 2 Parität und Spec-Abdeckung | `scripts/gate/parity_coverage.sh <ausgabe-ordner>` (liest `<ausgabe-ordner>/cargo-test.txt` aus Schritt 1) |
| 3 CLI-E2E (Zielbild-Befehl aus `docs/plan/M1.md` 1.2) | `scripts/gate/cli_e2e.sh <ausgabe-ordner>` |
| 4 MCP-E2E | `scripts/gate/mcp_e2e.sh <ausgabe-ordner>` |

Jedes Skript beendet sich mit dem Exit-Code des geprüften Befehls (0 = grün), schreibt seine Ausgabe in den Ordner und
braucht kein Netz außer dem lokalen Testserver. Ablauf- und Arbeitsordner liegen außerhalb des Repos.
