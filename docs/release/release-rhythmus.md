# Wöchentlicher Release-Rhythmus

Beschlossen am 04.11.2026 (Nutzerwunsch, Termine vom Nutzer bestätigt): Ab der Woche vom 09.11.2026 erscheinen Updates gesammelt einmal pro Woche am **Donnerstag** statt einzeln zwischendurch. Veröffentlicht wird weiterhin nur nach ausdrücklichem Go des Nutzers (siehe `checkliste-release.md`).

## Wochenablauf

| Tag | Was passiert |
|---|---|
| Freitag – Dienstag | Pakete werden gebaut, von Fable abgenommen und nach `main` gemergt. `main` bleibt jederzeit veröffentlichbar; halbfertige Funktionen bleiben im Paket-Branch. |
| **Dienstag 18:00** | **Merge-Schluss.** Was danach fertig wird, geht in die Woche darauf. Ausnahme: Korrekturen am Release-Kandidaten selbst. |
| Mittwoch Vormittag | Release-Kandidat: `set-version`, CHANGELOG-Abschnitt und Update-Notes festlegen (die Notes landen beim Bauen in `latest.json` und lassen sich danach nicht mehr ändern), Release-Workflow erzeugt den Entwurf. |
| Mittwoch | Prüfungen am Entwurf: Update von der Vorwoche auf den Kandidaten (Mac mini, Windows), Installations-Smoke, kurzer 100k-Benchmark gegen die Vorwoche, Website-Änderungen der Woche mit `check-site` vorbereitet. |
| Mittwoch Abend | Zusammenfassung an den Nutzer: Inhalt, Testergebnisse, Text der Release-Notes, offene Risiken. |
| **Donnerstag 10:00** | Nach Go: Entwurf als Latest veröffentlichen, Download-Links und Manifeste prüfen, Website (Handbuch, Funktionsseiten) zur neuen Version deployen, Ankündigung in GitHub Discussions (eigenes Go). |
| Donnerstag – Freitag | Absturzberichte, Issues und Discussions beobachten; Freitag ist Puffer für einen Hotfix. Kein Release am Wochenende. |

## Versionen

- Eine Beta pro Woche: beta.3 am 12.11.2026, beta.4 am 19.11.2026 usw., bis 1.0.
- Fällt eine Woche ohne nennenswerte Änderungen aus, entfällt das Release; der Rhythmus bleibt.

## Hotfixes außerhalb des Rhythmus

Nur bei Absturz beim Start oder Crawl, Datenverlust in Crawl-Dateien, Sicherheitslücke oder kaputtem Updater/Installer. Alles andere wartet bis Donnerstag. Versionsschema für Hotfixes noch zu klären (Vorschlag `1.0.0-beta.3.1`; vorher prüfen, dass der Updater es höher als beta.3 und niedriger als beta.4 einordnet).

## Website

Website-Änderungen, die neue Funktionen beschreiben, gehen mit dem Release am Donnerstag live. Reine Text- oder Fehlerkorrekturen dürfen jederzeit raus.

## Voraussetzungen (aus `docs/plan/beta3.md` §5/§6)

- Pfadfilter in der CI, damit Website-/Docs-Commits am Mittwoch keinen laufenden Kandidaten-Lauf abbrechen.
- Release-Notes stehen vor dem Bauen fest (Mittwoch Vormittag).
- Entwurfshinweise nur in die Actions-Zusammenfassung, nicht in den Release-Text.
- Wiederholung in `scripts/release/licenses/generate.mjs`, damit der Mittwochslauf nicht an einem Netz-Timeout hängt.
