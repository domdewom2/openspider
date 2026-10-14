# M2-Merkzettel (Backlog aus M1)

Stand: 2026-10-14 · Planer: Opus · Quelle: Fable-Gate M1 (needs_work, 70/100) und Fable-Gate Welle 5. Fables Empfehlungen sind übernommen (Nutzerentscheidung). Der Merkzettel sammelt Punkte, die Fable nach M2 verschoben hat. Keiner davon ist ein M1-Abnahmekriterium. Der M2-Plan übernimmt sie mit Spec-ID in seine Pakete. Querverweis: `docs/plan/M1.md` Abschnitt 7.3 (Nachtrag M1-Abnahme-Gate).

## 1 Beobachtungen und Messgrößen

| Nr. | Thema | Befund | Vorschlag für M2 | Bezug |
|---|---|---|---|---|
| M2-B01 | Windows-Durchsatzquote | `crawler_urls_per_s` in `loadtest-small` perf-001: windows-x64 lag in 2 von 3 main-Läufen nach P30 unter 50 % von macos-arm64 (49,3 %, 46,8 %; dazwischen 57,5 %; Merge-Lauf 38567144472: 55 %). Gegenüber macos-x64 liegt Windows bei 94–169 %. Die Schwankung kommt überwiegend von den macos-arm64-Läufen. | Quote je main-Lauf im Gate mitführen. Auf Windows bei IPv4-only-Servern den `localhost`-Verbindungsaufbau prüfen (Happy Eyeballs ~310 ms, eigener Connector in hyper-util; Welle-4c-Rückmeldung). Erst handeln, wenn der Median über 5 Läufe < 50 % liegt. | PERF-002 (M2), `docs/plan/M1-acceptance/checklist.md` A4 |
| M2-B02 | Anteil `verified` | Nur 5 von 242 M1-IDs tragen einen Paritätsbeleg (ONPAGE-012/-120/-130, TECH-010/-011). | Nach Welle 6 je Export mit erreichter Schwelle Regeln in `tests/parity/evidence.toml` ergänzen. Ziel: alle Tabs, deren Spalten vollständig verglichen werden. Sitemaps- und Hreflang-Filter bleiben wegen SF-FREE-01/02 ohne Beleg; der Grund ist eine Referenzlücke (K-53). Eine Pro-Lizenz der Referenz oder Listenmodus-Exporte würden sie schließen. | A9, K-53 |

## 2 Werkzeuge und Oberfläche

| Nr. | Thema | Befund | Vorschlag für M2 | Bezug |
|---|---|---|---|---|
| M2-T01 | `openspider mcp --crawl-dir` | Die Option erwartet den Crawl-Verwaltungsordner (Registry). Ein Ordner mit der per `--save-crawl` abgelegten `crawl.ospider` liefert in `list_crawls` 0 Crawls. Der Hilfetext „Ordner mit den Crawl-Dateien“ ist dafür missverständlich. | Hilfetext präzisieren („Crawl-Verwaltungsordner, siehe OUT-227“). Zusätzlich einen Ordner mit `*.ospider`-Dateien ohne Registry lesend unterstützen oder mit einer klaren Fehlermeldung abweisen. Neuer Test im MCP-Prozesstest. | AGENT-001/-003, OUT-227 |

## 3 Spec-Lücken aus dem Paritätslauf

| Nr. | Spec | Befund | Vorschlag für M2 |
|---|---|---|---|
| M2-S01 | TECH-051 (Directives) | Die Referenz exportiert `Indexability` und `Indexability Status` auch im Tab Directives; die Spaltenliste der Spec nennt sie nicht. Das Paritäts-Mapping ignoriert sie mit Grund. | Spaltenliste von TECH-051 um beide Spalten ergänzen (Position wie in der Referenz-Kopfzeile), `os-analyze` `tabs/directives.rs` nachziehen, Mapping-Ausnahme entfernen. |
| M2-S02 | TECH-071 (Hreflang) | Wie M2-S01 für den Tab Hreflang. | Spaltenliste von TECH-071 ergänzen, `tabs/hreflang.rs` nachziehen, Mapping-Ausnahme entfernen. |
| M2-S03 | TECH-130 (Links) | Die Spec sagt, `Inlinks` stehe nicht in der Referenz-Spaltenliste des Links-Tabs. Der Referenz-Export enthält die Spalte aber. OpenSpider blendet sie im Export aus. | Kriterium „Zusatzspalte, standardmäßig ausgeblendet“ in TECH-130 streichen und `Inlinks` sichtbar an der Position der Referenz führen. Mapping-Ausnahme entfernen; Self-Link-Zählung (K-07, DEV-010/-011) bleibt eine OS-Festlegung. |

## 4 Hinweise aus früheren Gates (zur Erinnerung)

- Windows: `localhost` gegen nur auf IPv4 lauschende Server (Welle-4c-Rückmeldung) → M2, Netzwerkschicht.
- `url_details::overview`, Zweig `ColumnSql::MultiPaired`, ist heute unerreichbar. Er wird bei späteren Tabs mit Mehrfachspalten genutzt.
- Fixture `ext-noindex.html` gehört zu keiner der fünf Paritäts-Sites und ist damit nicht gegen die Referenz prüfbar. Mit einer Erweiterung der Paritäts-Sites in M2 nachholen.
