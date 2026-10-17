# Bericht E1 Schattenbetrieb (M2-V02)

**Stand: Entwurf, VPS-Teil nicht ausgeführt.** Die Auslieferung (Skripte, Units, Konfiguration, Vergleich, Tests, Runbook) ist
fertig und lokal geprüft. Alles, was den VPS betrifft (Einrichten, Sofortlauf, Messwerte, Vergleichsergebnis), steht aus und ist
unten als „offen“ markiert. Der Grund: Das Paket verlangt Root-Änderungen am gemeinsam genutzten Produktivserver (Systembenutzer,
Units, `/opt`) und Lesezugriff auf Autonomo. Dafür lag in der Sitzung keine Freigabe des Nutzers im Chat vor, nur der Hinweis im
Plan (M2.md 3.13, „Nutzer-Go 16.10.2026“). Die Ausführung nach `E1-runbook.md` übernimmt der Orchestrator bzw. der Nutzer; dann
werden die Abschnitte 2 bis 6 ausgefüllt.

## 1. Aufbau (Auslieferung)

| Teil | Datei im Repo | Ziel auf dem VPS |
|---|---|---|
| Binärdatei | Artefakt `openspider-linux-x86_64` des letzten grünen `linux-core`-Laufs auf `main`, sha256 gegen `openspider.sha256` | `/opt/openspider-shadow/bin/openspider`, `bin/VERSION` (Commit, Lauf-ID, sha256, Datum) |
| Übertragung | `scripts/shadow/deploy.sh` (Vorher-/Nachher-Liste, kein Auto-Update) | – |
| Crawl-Konfiguration | `scripts/shadow/shadow.osconfig` | `etc/shadow.osconfig` |
| Umgebung | `scripts/shadow/shadow.env` (Sites, Aufbewahrung 4, Mindestplatz 8 GB, Quelle) | `etc/shadow.env` |
| Lauf | `scripts/shadow/run.sh` | `bin/run.sh` (ExecStart) |
| Vergleich | `scripts/shadow/compare.sh`, `compare.py`, `rule-map.toml`, `autonomo-source.toml` | `bin/`, `etc/` (ExecStartPost, mit `+` nur für den Lesezugang) |
| Units | `scripts/shadow/systemd/openspider-shadow.{service,timer}` | `systemd/`, eingebunden per `systemctl link` (nur Symlinks) |
| Rückbau | `scripts/shadow/uninstall.sh` (nur ausgeliefert) | `bin/uninstall.sh` |

Ablage auf dem VPS ausschließlich unter `/opt/openspider-shadow/` (`bin/`, `etc/`, `systemd/`, `var/{crawls,exports,reports,home}`),
Systembenutzer `openspider-shadow` (nologin). Kein Port, kein Caddy-, Docker-, Firewall- oder Cron-Eintrag.

Vorgaben, die Konfiguration und Unit umsetzen: 2 Threads, höchstens 2 URL/s, robots.txt respektieren (bei unerreichbarer
robots.txt Standardregel `block_host`), User-Agent und robots-Token „OpenSpider-Shadow“, kein Rendering, 20 000 URLs als Schutz,
Sites nacheinander. Unit: `Type=oneshot`, `Nice=15`, `IOSchedulingClass=idle`, `CPUQuota=200%`, `MemoryMax=2G`,
`RuntimeMaxSec=6h`, `NoNewPrivileges`, `ProtectSystem=strict`, `ReadWritePaths=/opt/openspider-shadow/var`, `PrivateTmp`,
`ProtectHome`. Timer: `Sun *-*-* 02:30:00 UTC`, `RandomizedDelaySec=15min`, `Persistent=true`.

**Vorher-Liste (`systemctl list-unit-files`, `list-timers`, `ss -ltn`, `df -h /`):** offen.
**Nachher-Liste und Unterschied:** offen. Erwartung: nur `openspider-shadow.service` und `.timer`.
**Version auf dem VPS:** offen.

## 2. Laufdaten je Site (Sofortlauf)

| | caloone.de | rotweine.org |
|---|---|---|
| URLs gecrawlt | offen | offen |
| Dauer | offen | offen |
| mittlere / höchste Rate (URL/s, Vorgabe ≤ 2) | offen | offen |
| Spitzen-RSS | offen | offen |
| Fehler (4xx/5xx/ohne Antwort) | offen | offen |
| durch robots.txt gesperrte URLs | offen | offen |

## 3. Regelzuordnung

`scripts/shadow/rule-map.toml` enthält das Format, die Prüfung gegen `M1-filters.toml`/`M2-filters.toml` und die Liste der
61 Befund-Filter, die der Sofortlauf unabhängig vom Stand der Zuordnung exportiert (dazu `Internal:All`, `Internal:HTML`). Die
`[[rule]]`-Einträge sind **offen**: Die `rule_id`-Kennungen von Autonomo sind nur auf dem VPS lesbar und wurden nicht
eingesehen; sie werden nicht geraten. Tabelle (Autonomo-Kennung, OpenSpider-Tab:Filter, Klasse gleichwertig / teilweise / ohne
Gegenstück, Begründung): offen.

## 4. Übereinstimmung je Regel

Je Regel Anzahl Autonomo, Anzahl OpenSpider, Schnittmenge, nur Autonomo, nur OpenSpider, Jaccard und Zeitabstand der beiden
Crawls: offen (`var/reports/<datum>/<site>.json|md`).

## 5. Abweichungen und Ursachenvermutung

Offen. Vorbereitet ist die Einordnung nach den Kategorien aus dem Plan: URL-Normalisierung (Regeln stehen im Kopf von
`compare.py`: Schema/Host klein, Fragment, Standardport, Prozent-Kodierung, Slash bleibt), Weiterleitungen,
Indexierbarkeit, robots, Seitenlimit (20 000), Zeitversatz zwischen den Crawls.

## 6. Offene Punkte für den Planer

1. **Freigabe und Ausführung des VPS-Teils** (siehe Kopf dieses Berichts, Runbook Schritte 2 bis 6).
2. **Autonomo-Schnittstelle ungeprüft:** Pfade, Feldnamen und Abfragen in `autonomo-source.toml` sind Annahmen
   (`verified = false`, der Abruf bricht bis zur lesenden Prüfung per `discover` ab). Offen ist auch, ob caloone.de und rotweine.org
   (eigene Domains) über `/api/extern` erreichbar sind oder nur per Datenbank-Lesesitzung.
3. **Zuordnungstabelle** braucht die Autonomo-Kennungen (Abschnitt 3).
4. **Logfile-Analyse in Autonomo:** Die Schattencrawls erscheinen dort als unbekannter Bot „OpenSpider-Shadow“. Der Bot-Katalog
   wird nicht geändert (Vorgabe); die Zahlen der Logauswertung für die beiden Sites sind in den Lauffenstern daher leicht erhöht.
5. **Spec-/Autonomo-Befunde:** nach dem Vergleich (noch keine).
6. Das Verzeichnis `scripts/shadow/tests` (44 Python-Tests) läuft in keinem CI-Job; die Workflows gehören anderen Paketen
   (`linux-core.yml`: M2-F02). Vorschlag an den Planer: einen Schritt `python3 -I -m unittest discover -s scripts/shadow/tests`
   in einen bestehenden Linux-Job aufnehmen.

## 7. Lokale Prüfung der Auslieferung (17.10.2026, macOS arm64, Release-Build von `2f38c4a`)

- `shadow.osconfig` wird von der Binärdatei ohne Fehler und ohne Warnung geladen. Gegen einen lokalen Testserver
  (Loopback, 4 Seiten, `robots.txt` mit eigener Gruppe für „OpenSpider-Shadow“) sendet der Crawl exakt den User-Agent
  `OpenSpider-Shadow`, beachtet die Gruppe des robots-Tokens (gesperrte Seite erscheint in
  `Response Codes:Internal Blocked by Robots.txt` mit Status 0) und legt 61 + 2 CSV-Exporte, `crawl.ospider` und die
  JSON-Fortschrittszeilen ab; mit `--no-store` bleibt `var/home/crawls` leer.
- `run.sh`: Sites nacheinander, `run.json` je Site, Aufbewahrung der letzten N Läufe, Abbruch mit Exit-Code 75 bei zu wenig
  Platz, Crawl-Fehler stehen in `run.json` (Test mit Attrappe).
- `compare.sh`/`compare.py`: Vergleich, JSON und Markdown, Rate/RSS-Auswertung, Token wird weder ausgegeben noch gespeichert,
  nur GET gegen Loopback (Test mit Aufzeichnungsserver), SELECT-Prüfung, `verified = false` verhindert jeden Abruf.
- Nicht lokal prüfbar (macOS): `VmHWM`-Messung, `setpriv`, systemd-Eigenschaften der Unit, `deploy.sh` (nicht ausgeführt).
