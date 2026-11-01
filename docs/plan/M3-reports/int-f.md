# M3 Integrationsrunde f – FIX-E (CRAWL-126)

## Stand und Umfang

01.11.2026, Branch `m3/int-f`. Basis `origin/m3/int-e` = `a2b7bd61`, integriert
`m3/fix-e` = `72012062` (32 Commits). Merge-Commit `869b18a1`:
`m3: Merge m3/fix-e (CRAWL-126 Pause mit Hintergrund-Merge)`. Kein Tag, keine
Veröffentlichung.

FIX-E baut direkt auf `m3/int-e` auf; der Merge ist konfliktfrei und sein Baum ist
identisch mit dem Baum von `72012062`. Inhalt und Begründung stehen in
`docs/plan/M3-reports/fix-e.md`: End-Batch mit `synchronous=FULL`, TRUNCATE-Checkpoint
wartet höchstens 100 ms auf Leser (Busy-Handler nach Wanduhrzeit), Rest als
abbrechbarer Hintergrund-Merge; Beenden wartet bis 3 s, „Im Finder zeigen“ bis 2 s
(`crawl-merge-pending`), Statuszeile „Pausiert – Datei wird zusammengeführt“.

## Prüfungen

Da der Baum identisch ist, gelten die Läufe auf `m3/fix-e-test`:

| Lauf | Stand | Ergebnis |
|---|---|---|
| CI 41303542855 (ohne Intel) | `72012062` | grün |
| ui-e2e 41287318605, Versuch 2 | `d9e96d9f` | grün auf allen vier Plattformen |

`72012062` ändert gegenüber `d9e96d9f` nur Unit-Tests und einen Kommentar in
`crates/os-engine/src/writer_thread.rs` sowie den Bericht. Die roten Specs im ersten
ui-e2e-Versuch (`n04-live.spec.ts:74`, `header.spec.ts:69`) sind vorbestehende
Testrennen und nicht durch FIX-E verursacht (Bisect in `fix-e.md`, Nachtrag 6).

## Freigabe

FIX-E: Fable APPROVED MIT AUFLAGEN (Reveal begrenzt), Auflage erfüllt (`deba2e77`).
Den „nicht ursächlich“-Befund zu den roten Specs hat Fable in der int-23-Prüfung
bestätigt.

## Folgearbeiten

- M2-Stand `main` `df02958d` (int-23, enthält den FIX-E-Port als Cherry-Picks) in
  einer späteren M3-Runde integrieren; die Unit-Tests aus `dd02fe65` kommen mit dem
  nächsten M2-Port.
- Testrennen `header.spec.ts:69` und `n04-live.spec.ts:74` an die Spec-Besitzer
  (Erwartungsänderungen nur nach K2-63).
- CLI-Crawls ohne Hintergrund-Merge können nach Pause oder Ende eine `-wal`-Datei
  hinterlassen, wenn ein fremder Leser den Checkpoint blockiert.
