# M2-Merkzettel (Backlog aus M1)

Stand: 2026-10-15 (Nachtrag M2-S04 … M2-S06 aus K-66 … K-71) · Planer: Opus · Quelle: Fable-Gate M1 (needs_work, 70/100) und Fable-Gate Welle 5. Fables Empfehlungen sind übernommen (Nutzerentscheidung). Der Merkzettel sammelt Punkte, die Fable nach M2 verschoben hat. Keiner davon ist ein M1-Abnahmekriterium. Der M2-Plan übernimmt sie mit Spec-ID in seine Pakete. Querverweis: `docs/plan/M1.md` Abschnitt 7.3 (Nachtrag M1-Abnahme-Gate).

## 1 Beobachtungen und Messgrößen

| Nr. | Thema | Befund | Vorschlag für M2 | Bezug |
|---|---|---|---|---|
| M2-B01 | Windows-Durchsatzquote | `crawler_urls_per_s` in `loadtest-small` perf-001: windows-x64 lag in 2 von 3 main-Läufen nach P30 unter 50 % von macos-arm64 (49,3 %, 46,8 %; dazwischen 57,5 %; Merge-Lauf 38567144472: 55 %). Gegenüber macos-x64 liegt Windows bei 94–169 %. Die Schwankung kommt überwiegend von den macos-arm64-Läufen. | Quote je main-Lauf im Gate mitführen. Auf Windows bei IPv4-only-Servern den `localhost`-Verbindungsaufbau prüfen (Happy Eyeballs ~310 ms, eigener Connector in hyper-util; Welle-4c-Rückmeldung). Erst handeln, wenn der Median über 5 Läufe < 50 % liegt. | PERF-002 (M2), `docs/plan/M1-acceptance/checklist.md` A4 |
| M2-B02 | Anteil `verified` | Nur 5 von 242 M1-IDs tragen einen Paritätsbeleg (ONPAGE-012/-120/-130, TECH-010/-011). | Nach Welle 6 je Export mit erreichter Schwelle Regeln in `tests/parity/evidence.toml` ergänzen. Ziel: alle Tabs, deren Spalten vollständig verglichen werden. Sitemaps- und Hreflang-Filter bleiben wegen REF-FREE-01/02 ohne Beleg; der Grund ist eine Referenzlücke (K-53). Eine Pro-Lizenz der Referenz oder Listenmodus-Exporte würden sie schließen. | A9, K-53 |

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
| M2-S04 | ONPAGE-005 (Duplikate, A-4b) | M1-P31 vergleicht mit der Kollation `OS_TEXT` (ICU-Root, Secondary) statt `str::to_lowercase`, weil `os-mcp` den Filterplan auf Verbindungen ausführt, die nur `OS_TEXT` kennen; die Gruppierung läuft als unkorrelierte Unterabfrage ohne Index (K-70). Gegen die Referenz belegt ist nur reine Groß-/Kleinschreibung. | Verifikationsauftrag: Paritäts-Fixture mit „Straße“/„STRASSE“, NFC-/NFD-„é“ und Sigma-Formen; danach A-4b bestätigen oder auf `to_lowercase` umstellen. Für große Crawls eine gefaltete Schlüsselspalte mit Index (Migration `os-store`) oder die Kollation `OS_FOLD` in `os-store` `open.rs` registrieren, damit jede Verbindung sie kennt. |
| M2-S05 | TECH-011/TECH-015 (Canonicals, DEV-035) | Die Referenz führt in Canonicals „Internal HTML or PDF pages with 2xx“ und zählt PDFs ohne Canonical als Missing; OpenSpider führt Nicht-HTML nur mit HTTP-Canonical (OS-Festlegung für M1, K-71). | Mit ONPAGE-166 (PDF als Seite) Grundmenge und Missing an die Referenz angleichen, DEV-035 streichen. |
| M2-S06 | ONPAGE-060 (A-19b), ONPAGE-141 (A-35b) | Unbelegt gegen die Referenz: Vorlauf einer Schleife als Redirect Chain (A-19b, K-66) und `<li>` ohne Link im Word Count (A-35b, K-68). | Paritäts-Sites um je einen Fall erweitern (Vorlauf `/tail` → Schleife; Liste ohne Links im Inhaltsbereich), Annahmen bestätigen oder korrigieren. |

## 4 Hinweise aus früheren Gates (zur Erinnerung)

- Windows: `localhost` gegen nur auf IPv4 lauschende Server (Welle-4c-Rückmeldung) → M2, Netzwerkschicht.
- `url_details::overview`, Zweig `ColumnSql::MultiPaired`, ist heute unerreichbar. Er wird bei späteren Tabs mit Mehrfachspalten genutzt.
- Fixture `ext-noindex.html` gehört zu keiner der fünf Paritäts-Sites und ist damit nicht gegen die Referenz prüfbar. Mit einer Erweiterung der Paritäts-Sites in M2 nachholen.
