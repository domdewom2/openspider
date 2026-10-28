# Beta-Abnahme M2-β (M2-A02): Build

Stand 28.10.2026, 07:00. App aus `main` 572e14db (Runde 12 + Runde 13 + Heftungs-Wiederholung in `release-macos.yml`;
Inhalt der Anwendung identisch mit edf814f0).

| Feld | Wert |
|---|---|
| Commit | 572e14db01bc5065031750681f8adca7580291dd |
| Build-Weg | GitHub Actions `release-macos.yml` (`workflow_dispatch`, arch=universal, intel=false), Lauf 40624897076, signiert mit Developer-ID, notarisiert und geheftet |
| Artefakt | `openspider-macos-universal` → `OpenSpider_0.1.0_universal.dmg` (70,7 MB, Universal arm64 + x86_64) |
| SHA-256 (DMG) | d80752c447e29f798921dc4bf720be69526a403ce0c875a0a268912c68318cfa |
| Lokale Prüfung | `xcrun stapler validate` OK; `spctl --assess --type open --context context:primary-signature` → accepted, source „Notarized Developer ID“; Prüfskripte verify-app/verify-dmg: „ERGEBNIS: BESTANDEN“ |
| Windows-Installer | ausgelassen (Nutzerentscheidung 27.10.: Windows-Rechner vorerst nicht) |

Öffnen: DMG mounten, App nach „Programme“ ziehen, normal starten (kein Rechtsklick → Öffnen nötig, da notarisiert).
Erster Start legt den Einstellungsordner unter dem Benutzerprofil an; für die Abnahme ist kein vorhandener Crawl nötig.
