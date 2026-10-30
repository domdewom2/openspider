# U32 Tastatursuite

178 Fälle in Chromium/WebKit: je Browser die Mac- und Windows-Belegung des vollständigen Registers, Textfeld-/Dialogabgrenzung, Backspace-/Kontextmenü-Aliase, Kollisionen und Registerabgleich im echten Hilfedialog. Jedes Kürzel muss genau ein erwartetes Aktionsereignis senden. Die Handler sind Fixture-Endpunkte; diese Suite prüft den Produktions-Dispatcher, keine nativen NSMenu-Accelerators oder OS-Zwischenablage.

```sh
nice -n 10 timeout 3600s npx playwright test -c apps/desktop/e2e/keyboard-suite/playwright.config.ts --workers=4
```

Native Gegenprobe: E2E-B `s3` auf macOS/Windows, zusätzlich E2E-C auf macOS. Befehle stehen in `docs/plan/M2-acceptance/e2e/ausfuehrung.md`. Der Suite-Server reserviert einen ephemeren Loopback-Port und verweigert die Nutzung eines bereits laufenden Fremdservers.
