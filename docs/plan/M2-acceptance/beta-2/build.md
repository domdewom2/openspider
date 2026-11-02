# Zweite Nutzer-Runde M2-A03: Build

Stand: 31.10.2026, 17:30 MEZ. App-Code identisch mit `main` 322a2328; gebaut von `m2/fix-rel` (= main + Release-Fix „App-Ticket heften“ und Test-Zeitlimit, keine Produktänderung).

| Feld | Wert |
|---|---|
| Commit | 7fb4cc57b458777d9f47c00297474c34b295b512 |
| Build-Weg | GitHub Actions `release-macos.yml`, `workflow_dispatch`, arch=universal, intel=false; signiert (Developer ID), App und DMG notarisiert und geheftet |
| Lauf-ID | 41211714021 (vorheriger Lauf 41206175241 aus main verworfen: App im DMG ohne geheftetes Ticket) |
| Artefakt / DMG-Dateiname | openspider-macos-universal / OpenSpider_0.1.0_universal.dmg |
| Größe | 70.638.367 Byte (67,4 MiB) |
| SHA-256 (DMG) | cdca8477a519f0053bab17d5dce91e12f666b4a375ce98c2f762372997091da7 |
| Lokale Prüfung | DMG: `stapler validate` ok, `spctl -a -t open` accepted (Notarized Developer ID). App im DMG: `stapler validate` ok, `spctl -a -t exec` accepted, `lipo` x86_64 arm64, Bundle-ID org.openspider.desktop, Version 0.1.0. CI-Prüfskript verify-macos.sh im Lauf grün |
| Windows-Installer | ausgelassen: A03 läuft auf dem Mac; Windows-Installer sind nativ über die Release-/Smoke-Läufe nachgewiesen |

Öffnen: notarisiert und geheftet – DMG doppelklicken, App in „Programme“ ziehen, normal öffnen (auch offline ohne Warnung).

## Aktualisierter Build (31.10.2026, 23:40 MEZ) – für die Runde am 02.11. maßgeblich

App aus `main` bc964d71 (int-22: Speicherbudget N15F, Gegendruck N20, Diagnose N19; Release-Fix enthalten). Lauf 41238239864 (`release-macos.yml`, universal, intel=false; erster Versuch scheiterte am Heften des DMG-Tickets mit NSURLErrorDomain -1005, Wiederholung grün). DMG OpenSpider_0.1.0_universal.dmg, 70.666.255 Byte, SHA-256 `45c2730f9156e5194203138f8fb6f0811c5e297abab9568bc7c545df1f2f76d5`. Lokale Prüfung: DMG und App im DMG `stapler validate` ok, `spctl` accepted (Notarized Developer ID), `lipo` x86_64 arm64, Version 0.1.0. Der obige Build (Lauf 41211714021) bleibt als Vergleich erhalten.
