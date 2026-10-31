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
