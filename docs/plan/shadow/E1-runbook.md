# Runbook Etappe E1 Schattenbetrieb (M2-V02)

Für den Orchestrator (M2.md C11, lokal auf dem Mac, Zugang `ssh -o BatchMode=yes root@100.67.209.49`). Alle Dateien liegen unter
`scripts/shadow/`. Jeder Schritt ist wiederholbar; nichts davon läuft automatisch. Verbindlich bleibt M2.md 3.13.

Grundsätze: Auf dem VPS entsteht nur `/opt/openspider-shadow/` (plus Systembenutzer `openspider-shadow` und die Symlinks aus
`systemctl link`). Autonomo, evnxt, Caddy, Docker und alle bestehenden Units, Timer und Konfigurationen bleiben unverändert;
Autonomo wird nur gelesen (GET bzw. SELECT, kein POST/PUT/PATCH/DELETE, kein Auslösen von Crawls oder Rechecks).

## 1. Vorbereitung lokal

```
python3 -I -m unittest discover -s scripts/shadow/tests          # Python-Tests, kein Netz
scripts/shadow/deploy.sh --dry-run                               # lädt das Artefakt, prüft sha256, zeigt den Inhalt
```

Das Artefakt `openspider-linux-x86_64` stammt aus dem letzten grünen Lauf von `linux-core` auf `main` (`gh run download`,
`shasum -a 256 -c openspider.sha256`). Mit `--run-id <id>` lässt sich ein bestimmter Lauf wählen.

## 2. Einrichten

```
scripts/shadow/deploy.sh --out ~/shadow-e1          # schreibt vorher.txt, nachher.txt, unterschied.diff
```

Vor jedem (auch jedem erneuten) Aufruf prüft `deploy.sh` selbst `systemctl is-active openspider-shadow.service` und bricht ohne
Änderung ab, solange der Dienst läuft (sonst wäre die Binärdatei in Benutzung). Dann das Ende des Laufs abwarten
(`journalctl -fu openspider-shadow.service`) und erneut starten. Dateien in `bin/` werden per Kopie neben das Ziel und `mv`
atomar ersetzt.

`deploy.sh` holt die Vorher-Liste (`systemctl list-unit-files`, `list-timers`, `ss -ltn`, `df -h /`), legt Benutzer, Ordner,
Binärdatei (`bin/VERSION` mit Commit, Lauf-ID, sha256, Datum), Konfiguration und Units an, bindet die Units per
`systemctl link` ein und holt die Nachher-Liste. Der Timer bleibt bis Schritt 5 aus. Erwartung an `unterschied.diff`:
nur `openspider-shadow.service` und `openspider-shadow.timer` (jeweils `linked`/`disabled`), sonst nichts.

## 3. Lesezugang zu Autonomo prüfen (nur lesend)

Auf dem VPS, als root:

```
cd /opt/openspider-shadow
bin/compare.py discover --config etc/autonomo-source.toml                       # Struktur der Crawl-Liste, ohne Werte
bin/compare.py discover --config etc/autonomo-source.toml --path <Befund-Pfad>  # Struktur der Befund-Antwort
```

Die Pfade und Feldnamen in `etc/autonomo-source.toml` sind Annahmen. Passen sie, `[api] verified = true` setzen. Sind
caloone.de und rotweine.org über `/api/extern` nicht erreichbar, den Rückfall `[psql]` ausfüllen (genau ein SELECT je Abfrage,
Sitzung mit `default_transaction_read_only=on`, keine neue Rolle, kein Objekt), `verified = true` setzen und in
`etc/shadow.env` `SHADOW_AUTONOMO_SOURCE=psql` stellen. Die Kopie unter `etc/` bleibt bei späteren `deploy.sh`-Läufen erhalten
(neue Fassung als `autonomo-source.toml.dist`). Der Token wird nur gelesen, nie angezeigt oder kopiert.

## 4. Zuordnungstabelle füllen

Die `rule_id`-Kennungen von Autonomo sind nur auf dem VPS sichtbar. Liste der Regeln mit Befundzahlen aus dem ersten
Vergleich (`autonomo_unmapped` in `var/reports/<datum>/<site>.json`) oder aus der Datenbank (nur Kennungen). Für jede Regel
einen Eintrag in `scripts/shadow/rule-map.toml` (`autonomo`, `openspider`, `class`, `reason`) ergänzen und prüfen:

```
python3 scripts/shadow/compare.py validate --rule-map scripts/shadow/rule-map.toml \
  --filters docs/plan/M1-filters.toml docs/plan/M2-filters.toml
```

Keine Texte, Beschreibungen oder Listen aus Autonomo-Dokumenten übernehmen; Autonomo-Unterlagen mit Recherche zum
Referenzprodukt nicht lesen. Danach `deploy.sh` erneut ausführen (gleiche oder neue Artefakt-Version); vorher mit `systemctl is-active
openspider-shadow.service` sicherstellen, dass kein Lauf aktiv ist (`deploy.sh` bricht sonst selbst ab).

## 5. Sofortlauf, beobachtet, danach Timer

```
ssh root@100.67.209.49 systemctl start --no-block openspider-shadow.service
ssh root@100.67.209.49 journalctl -fu openspider-shadow.service        # bis „Deactivated successfully“
ssh root@100.67.209.49 systemd-cgtop -n 1 --depth=1 | grep openspider  # Stichproben während des Laufs
```

Nach dem Ende: `systemctl status openspider-shadow.service` (Exit-Status von `run.sh` und `compare.sh`), dann

```
ssh root@100.67.209.49 systemctl enable --now openspider-shadow.timer
ssh root@100.67.209.49 systemctl list-timers openspider-shadow.timer
scripts/shadow/deploy.sh --snapshot nachher.txt --out ~/shadow-e1       # Nachher-Liste mit aktivem Timer
```

Nur den Vergleich wiederholen (ohne neuen Crawl), z. B. nach einer ergänzten Zuordnungstabelle: auf dem VPS als root
`SHADOW_BASE=/opt/openspider-shadow SHADOW_AUTONOMO_SOURCE=api /opt/openspider-shadow/bin/compare.sh` (nutzt
`var/latest-run.json`).

## 6. Messwerte und Bericht

Je Site in `var/reports/<datum>/<site>.json` unter `metrics`: URLs (`urls_total` = Zeilen von `Internal:All`, das ist "URLs gecrawlt" im Bericht; `urls_crawled` aus der Fortschrittsdatei nur als Vergleichswert, ohne robots-gesperrte URLs), Dauer, mittlere Rate, Höchstrate (gemeldet und im
10-s-Fenster), Spitzen-RSS (`VmHWM` aus `run.json`). Vorgabe: ≤ 2 URL/s. Fehler und robots-Sperren aus
`var/exports/<site>/<stempel>/response_codes_*.csv` (Zeilenzahlen) und der Fortschrittsdatei
`var/crawls/<site>/<stempel>.progress.log`. Ergebnisse in `docs/plan/shadow/E1-bericht.md` eintragen (Auszüge, Rohdaten bleiben auf
dem VPS), dann auf `m2/m2-v02` committen.

## 7. Rückbau

`/opt/openspider-shadow/bin/uninstall.sh` (auf dem VPS, `ssh -t`, als root): stoppt Timer und Dienst, entlinkt die Units und
fragt vor dem Löschen von `/opt/openspider-shadow` und des Benutzers einzeln nach. Wird nur ausgeliefert, nicht ausgeführt.
