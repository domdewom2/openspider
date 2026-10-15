# Kurzproben M1-P33: Speicher der Engine bei großen Crawls (K-78, K-74)

Belege für `M1-P33` (Welle 7): RSS-Trend von `perf-003` bei 100 000 / 250 000 / 500 000 Seiten, Durchsatz von `perf-001` und `perf-003` bei 250 000 Seiten, `perf-005` (harte Abbrüche) und Pausieren/Fortsetzen mit 1 Mio. Warteschlangeneinträgen. Der 1-Mio.-Beleg selbst kommt aus Welle 7b (M1-N21); hier steht der **Trend**, die Aufschlüsselung der Posten und der A/B-Vergleich gegen den Ausgangsstand. Absolutwerte sind nur im Vergleich unter gleicher Last belastbar.

## Ergebnis in Kürze

| Kriterium | Ziel | Messwert (Endstand S4) | erfüllt |
|---|---|---|---|
| Hochrechnung des Spitzen-RSS `perf-003` auf 1 Mio. Seiten (lineare Fortschreibung der Läufe bei 250 000 und 500 000 Seiten) | <= 470 MB | **459 MB** (433 MB bei 500 000 + 5 x 5,2 MB je 100 000; Ausgangsstand: 672 MB = 538 + 5 x 26,8) | ja, knapp (Unsicherheit der Läufe rund ±20 MB, siehe Befund 7) |
| Spitzen-RSS `perf-003` bei 100 000 / 250 000 / 500 000 Seiten | flacher als im Ausgangsstand | 408 / 420 (Median aus 5) / 433 MB; Ausgangsstand 424 / 471 (Median aus 10) / 538 MB | ja: Steigung 5,2 statt 26,8 MB je 100 000 Seiten |
| Strukturen der Engine bei 500 000 Seiten (DIAG-Zähler, Spitze) | Aufschlüsselung | 17,5 MB (Seen 12,6, Warteschlange im RAM 4,5, Sortierstruktur 0,8) statt 84,8 MB (Seen 22,9, Warteschlange 49,3, Sortierstruktur 12,6) | - |
| Durchsatz `perf-001` 250 000 Seiten | nicht schlechter als -5 % gegen den Ausgangsstand | 2 Paare im selben Block: +2 %, +2 %; Zeit ohne Writer unverändert | ja (Streuung der Läufe ±20 %, Tabelle unten) |
| Durchsatz `perf-003` 250 000 Seiten | nicht schlechter als -5 % | 5 Paare (S4 gegen S0): -60 %, -1 %, +24 %, -33 %, -1 %; die Abweichungen nach unten liegen in der Zeit des Writers (+216 s, +87 s, jeweils als erster Lauf des Blocks), die Zeit ohne Writer ist gleich | in 3 von 5 Paaren und im Mittel der Paare mit S4 an zweiter Stelle ja; Engine-Anteil unverändert (Befund 4), Gesamtdurchsatz wegen der Streuung des Writers nicht schärfer entscheidbar |
| Pausieren / Fortsetzen mit 1 Mio. Warteschlange (CRAWL-126, `lifecycle_pause --ignored`) | Pausieren < 3 s | Pausieren 10 bis 12 ms (vorher 71 bis 75 ms), Fortsetzen 0,66 s (vorher 1,13 bis 1,16 s), Spitzen-RSS des Testprozesses 387 MB (vorher 704 MB) | ja |
| PERF-005 (K-49): 5 harte Abbrüche, 100 000 Seiten | 0 Dubletten, 0 fehlend, `integrity_check` ok, wiederholte Requests <= 2 264 je Abbruch | 0 / 0 / ok; Mittel 1703 | ja |
| Tests | fmt, clippy `-D warnings`, `cargo test --workspace --exclude openspider-desktop --locked` | `cargo fmt --all --check` und clippy ohne Befund; 2 005 bestanden, 0 fehlgeschlagen, 34 ignoriert; `cargo deny check`: bans, licenses, sources ok; advisories schlägt fehl (rustls RUSTSEC-2026-0346 bis -0349, `ttf-parser` ohne Pflege), das gilt unverändert für `main` (keine Abhängigkeit geändert) | ja, advisories vorbestehend |

## Stände und Messweise

| Kürzel | Stand | Inhalt |
|---|---|---|
| S0 | `b50ca47` | Ausgangsstand: Verhalten wie `origin/main` (46b8d6f), dazu nur die Speicherzähler im DIAG-Block |
| S1 | `9be00ec` | kompakte Seen-Menge (K-78 a) mit 256 `HashMap`-Teiltabellen |
| S1b | wie S1, 16 Teiltabellen | nur Zwischenbefund, nicht committet |
| S2 | `de626ba` | wie S1, dazu RAM-Fenster der Warteschlange W = 20 000 (K-78 b) und Fortsetzen ohne `links_target` (K-74) |
| S3 | `a6c386b` | wie S2 mit 16 statt 256 Teiltabellen |
| S4 | `6080f7f` | **Endstand:** Seen-Menge als kompakte Tabelle mit offener Adressierung, Fenster W = 10 000 |

- Jeder Stand ist ein Release-Build von `os-loadtest` (`cargo build --release --locked -p os-loadtest`, `RUSTC_WRAPPER=sccache`, `CARGO_INCREMENTAL=0`). Berichte, Konsolenausgaben und RSS-Verläufe (alle 10 s, `rss-*.csv`) je Stand liegen in `s0-baseline/`, `s1-seen/`, `s1b-seen16/`, `s2-window/`, `s3-final/`, `s4-final/` (Dateiname `report-<szenario>-<seiten>[-rN].json`; ohne Suffix der erste Lauf). `s2-example/` und `s4-example/` enthalten `perf-005` (braucht das Beispielprogramm `crawl_child` neben der Binärdatei).
- Befehl je Lauf: `os-loadtest run --scenario perf-003|perf-001|perf-005 --size large --pages <N> --work-dir <Ordner außerhalb des Repos> --out …` (8 Threads, Profil `balanced`, bei `perf-003` Senke mit 100 ms je Schreibaufruf), unter `caffeinate -dimsu`. Die Crawl-Dateien wurden nach jedem Lauf gelöscht (frei waren vorher mindestens 22 GB).
- **Messruhe (Welle 7):** Jeder Block lief unter `lockf -t 7200 …/bench.lock`, vorher `scripts/bench/wait_quiet.sh 8 10` (Grenze 8, höchstens 10 min). Gebaut und getestet wurde ausschließlich außerhalb der Sperre. Die Load (1 min) zu Beginn und Ende jedes Laufs steht in `block*-log.txt`; sie lag bei 2 bis 5, nur zu Beginn von Block D bei 7,8 (im Hintergrund lief noch `cargo test` des Pakets) und kurz in einzelnen Läufen darüber; der Lauf selbst belegt rund 3 Kerne. In allen Blöcken wurde die Ruhe (Load < 8) erreicht.
- **A/B:** Die Stände laufen unmittelbar nacheinander im selben Block, die Reihenfolge wechselt zwischen den Blöcken. Die Streuung einzelner Läufe ist groß: Der Durchsatz derselben Binärdatei schwankt um ±20 %, weil über 85 % der Laufzeit auf den Writer entfallen (`engine_w_write_ns`, Commits und Checkpoints der Datei) und diese Zeit je Lauf stark schwankt; der Spitzen-RSS schwankt um bis zu ±20 MB. Deshalb stehen mehrere Läufe je Stand, der Median und, wo es um die Engine geht, die Zeit **ohne** den Writer („Rest“).

## Blöcke

| Block | Inhalt |
|---|---|
| A | `perf-003` 100 000 und 250 000 Seiten, S0, S1, S2 |
| B1, B2 | `perf-003` 250 000 Seiten, Wiederholungen (Reihenfolge S2-S1-S0 und S1-S0-S2) |
| C, C2 | `perf-001` 250 000 Seiten, S0, S1, S2 (C2 in umgekehrter Reihenfolge, in einer unruhigeren Phase der Maschine) |
| D | `perf-003` 500 000 Seiten, S0, S1, S2 (mit RSS-Verlauf) |
| E2, I6 | `perf-005` 100 000 Seiten, S2 und S4 (E: erster Versuch ohne Beispielprogramm, verworfen) |
| F, F2 | `perf-003` 250 000 Seiten, S1b (16 Teiltabellen) |
| G, G2 | `lifecycle_pause` mit 1 Mio. Warteschlangeneinträgen, S1 gegen S3 (G) und S4 (G2) |
| H1 bis H4 | S3: `perf-003` 250 000 (zweimal gepaart mit S0), `perf-001` 250 000, 100 000 und 500 000 |
| I1 bis I5 | S4: `perf-003` 250 000 (zweimal gepaart mit S0), `perf-001` 250 000, 100 000 und 500 000 |
| J1 bis J4 | weitere gepaarte Läufe S4 gegen S0 (`perf-003` und `perf-001`, 250 000) |

## Tabellen

### RSS-Trend perf-003 (Spitzen-RSS in MB, je Lauf; Median)

| Stand | 100 000 | 250 000 | 500 000 | Steigung 250 k -> 500 k je 100 k | Hochrechnung 1 Mio. |
|---|---|---|---|---:|---:|
| S0 Ausgangsstand | 424 | 471, 475, 484, 470, 468, 469, 470, 471, 469, 479 (Median 471) | 538 | 26,8 | 672 |
| S1 Seen-Menge (256 Teiltabellen) | 425 | 628, 548, 622 (Median 622) | 701 | 31,5 | 859 |
| S1b wie S1 mit 16 Teiltabellen | - | 454, 456 (Median 455) | - | - | - |
| S2 + RAM-Fenster (256 Teiltabellen) | 412 | 437, 465, 479 (Median 465) | 460 | -2,2 | 449 |
| S3 wie S2 mit 16 Teiltabellen | 412 | 428, 425 (Median 427) | 446 | 7,7 | 485 |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 408 | 420, 415, 427, 417, 428 (Median 420) | 433 | 5,2 | 459 |


### Durchsatz und Stillstand perf-003

| Stand | Seiten | Lauf | Dauer s | URLs/s | davon Schreiben (Writer) s | Rest s | Stillstand s | Spitzen-RSS MB | bestanden |
|---|---:|---|---:|---:|---:|---:|---:|---:|---|
| S0 Ausgangsstand | 100000 | r1 | 41,4 | 2418 | 35,0 | 6,4 | 1,7 | 424 | ja |
| S1 Seen-Menge (256 Teiltabellen) | 100000 | r1 | 40,5 | 2466 | 33,8 | 6,7 | 2,0 | 425 | ja |
| S2 + RAM-Fenster (256 Teiltabellen) | 100000 | r1 | 39,0 | 2566 | 31,7 | 7,2 | 1,4 | 412 | ja |
| S3 wie S2 mit 16 Teiltabellen | 100000 | r2 | 37,8 | 2643 | 31,4 | 6,5 | 1,2 | 412 | ja |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 100000 | r3 | 37,5 | 2664 | 31,2 | 6,3 | 1,1 | 408 | ja |
| S0 Ausgangsstand | 250000 | r1 | 201,0 | 1244 | 183,5 | 17,5 | 6,5 | 471 | ja |
| S0 Ausgangsstand | 250000 | r2 | 156,1 | 1602 | 140,5 | 15,5 | 4,2 | 475 | ja |
| S0 Ausgangsstand | 250000 | r3 | 174,2 | 1435 | 156,6 | 17,6 | 5,2 | 484 | ja |
| S0 Ausgangsstand | 250000 | r4 | 155,2 | 1611 | 137,8 | 17,4 | 4,7 | 470 | ja |
| S0 Ausgangsstand | 250000 | r5 | 153,5 | 1629 | 137,3 | 16,1 | 3,2 | 468 | ja |
| S0 Ausgangsstand | 250000 | r6 | 149,3 | 1674 | 132,2 | 17,1 | 3,2 | 469 | ja |
| S0 Ausgangsstand | 250000 | r7 | 142,5 | 1754 | 124,8 | 17,7 | 2,7 | 470 | ja |
| S0 Ausgangsstand | 250000 | r8 | 189,3 | 1320 | 172,3 | 17,1 | 6,1 | 471 | ja |
| S0 Ausgangsstand | 250000 | r9 | 180,2 | 1387 | 162,5 | 17,7 | 5,4 | 469 | ja |
| S0 Ausgangsstand | 250000 | r10 | 152,6 | 1638 | 135,6 | 17,0 | 4,0 | 479 | ja |
| S1 Seen-Menge (256 Teiltabellen) | 250000 | r1 | 199,1 | 1255 | 179,7 | 19,4 | 6,1 | 628 | nein |
| S1 Seen-Menge (256 Teiltabellen) | 250000 | r2 | 189,4 | 1320 | 171,3 | 18,1 | 6,2 | 548 | nein |
| S1 Seen-Menge (256 Teiltabellen) | 250000 | r3 | 179,9 | 1390 | 164,3 | 15,6 | 4,5 | 622 | nein |
| S1b wie S1 mit 16 Teiltabellen | 250000 | r1 | 217,9 | 1147 | 199,8 | 18,1 | 9,8 | 454 | ja |
| S1b wie S1 mit 16 Teiltabellen | 250000 | r2 | 184,8 | 1353 | 167,6 | 17,2 | 11,0 | 456 | ja |
| S2 + RAM-Fenster (256 Teiltabellen) | 250000 | r1 | 160,9 | 1553 | 143,5 | 17,5 | 4,2 | 437 | ja |
| S2 + RAM-Fenster (256 Teiltabellen) | 250000 | r2 | 146,1 | 1711 | 129,1 | 17,0 | 2,8 | 465 | ja |
| S2 + RAM-Fenster (256 Teiltabellen) | 250000 | r3 | 198,6 | 1259 | 180,9 | 17,7 | 5,6 | 479 | ja |
| S3 wie S2 mit 16 Teiltabellen | 250000 | r4 | 182,6 | 1369 | 163,0 | 19,5 | 5,2 | 428 | ja |
| S3 wie S2 mit 16 Teiltabellen | 250000 | r5 | 212,9 | 1174 | 196,8 | 16,1 | 7,0 | 425 | ja |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 250000 | r6 | 370,7 | 674 | 348,4 | 22,3 | 12,0 | 420 | ja |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 250000 | r7 | 144,1 | 1735 | 127,3 | 16,7 | 2,9 | 415 | ja |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 250000 | r8 | 152,5 | 1639 | 135,1 | 17,4 | 3,9 | 427 | ja |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 250000 | r9 | 267,1 | 936 | 249,7 | 17,3 | 7,9 | 417 | ja |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 250000 | r10 | 154,0 | 1623 | 136,4 | 17,6 | 3,6 | 428 | ja |
| S0 Ausgangsstand | 500000 | r1 | 1455,4 | 344 | 1416,4 | 39,0 | 19,5 | 538 | nein |
| S1 Seen-Menge (256 Teiltabellen) | 500000 | r1 | 1466,2 | 341 | 1426,6 | 39,6 | 20,8 | 701 | nein |
| S2 + RAM-Fenster (256 Teiltabellen) | 500000 | r1 | 1093,3 | 457 | 1061,0 | 32,2 | 12,0 | 460 | ja |
| S3 wie S2 mit 16 Teiltabellen | 500000 | r1 | 1540,8 | 324 | 1498,0 | 42,8 | 21,4 | 446 | ja |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 500000 | r1 | 949,1 | 527 | 914,0 | 35,1 | 15,4 | 433 | ja |

Median URLs/s je Stand: S0 Ausgangsstand: 100000 -> 2418, 250000 -> 1606, 500000 -> 344; S1 Seen-Menge (256 Teiltabellen): 100000 -> 2466, 250000 -> 1320, 500000 -> 341; S1b wie S1 mit 16 Teiltabellen: 250000 -> 1250; S2 + RAM-Fenster (256 Teiltabellen): 100000 -> 2566, 250000 -> 1553, 500000 -> 457; S3 wie S2 mit 16 Teiltabellen: 100000 -> 2643, 250000 -> 1272, 500000 -> 324; S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000): 100000 -> 2664, 250000 -> 1623, 500000 -> 527


### Durchsatz und Stillstand perf-001

| Stand | Seiten | Lauf | Dauer s | URLs/s | davon Schreiben (Writer) s | Rest s | Stillstand s | Spitzen-RSS MB | bestanden |
|---|---:|---|---:|---:|---:|---:|---:|---:|---|
| S0 Ausgangsstand | 250000 | r1 | 187,5 | 1333 | 169,5 | 18,1 | - | 469 | ja |
| S0 Ausgangsstand | 250000 | r2 | 228,7 | 1093 | 210,6 | 18,1 | - | 470 | ja |
| S0 Ausgangsstand | 250000 | r3 | 170,4 | 1467 | 151,6 | 18,8 | - | 481 | ja |
| S0 Ausgangsstand | 250000 | r4 | 140,0 | 1786 | 122,7 | 17,3 | - | 470 | ja |
| S0 Ausgangsstand | 250000 | r5 | 154,4 | 1620 | 137,0 | 17,3 | - | 473 | ja |
| S1 Seen-Menge (256 Teiltabellen) | 250000 | r1 | 181,6 | 1377 | 162,2 | 19,4 | - | 556 | ja |
| S1 Seen-Menge (256 Teiltabellen) | 250000 | r2 | 300,8 | 831 | 277,9 | 23,0 | - | 461 | ja |
| S2 + RAM-Fenster (256 Teiltabellen) | 250000 | r1 | 187,3 | 1335 | 169,3 | 17,9 | - | 442 | ja |
| S2 + RAM-Fenster (256 Teiltabellen) | 250000 | r2 | 351,7 | 711 | 325,5 | 26,2 | - | 438 | ja |
| S3 wie S2 mit 16 Teiltabellen | 250000 | r3 | 173,8 | 1438 | 155,1 | 18,7 | - | 427 | ja |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 250000 | r4 | 137,3 | 1821 | 120,0 | 17,3 | - | 427 | ja |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 250000 | r5 | 150,9 | 1657 | 133,2 | 17,6 | - | 417 | ja |

Median URLs/s je Stand: S0 Ausgangsstand: 250000 -> 1467; S1 Seen-Menge (256 Teiltabellen): 250000 -> 1104; S1b wie S1 mit 16 Teiltabellen: ; S2 + RAM-Fenster (256 Teiltabellen): 250000 -> 1023; S3 wie S2 mit 16 Teiltabellen: 250000 -> 1438; S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000): 250000 -> 1739


### Gepaarte Läufe S4 gegen S0 (selber Block, unmittelbar nacheinander)

`perf-003` 250 000 Seiten:

| Lauf | S4 lief | S4 URLs/s | S0 URLs/s | Veränderung | S4 Writer s | S0 Writer s | S4 ohne Writer s | S0 ohne Writer s | S4 RSS MB | S0 RSS MB |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| r6 | erster | 674 | 1674 | -60 % | 348,4 | 132,2 | 22,3 | 17,1 | 420 | 469 |
| r7 | zweiter | 1735 | 1754 | -1 % | 127,3 | 124,8 | 16,7 | 17,7 | 415 | 470 |
| r8 | zweiter | 1639 | 1320 | +24 % | 135,1 | 172,3 | 17,4 | 17,1 | 427 | 471 |
| r9 | erster | 936 | 1387 | -33 % | 249,7 | 162,5 | 17,3 | 17,7 | 417 | 469 |
| r10 | zweiter | 1623 | 1638 | -1 % | 136,4 | 135,6 | 17,6 | 17,0 | 428 | 479 |

`perf-001` 250 000 Seiten:

| Lauf | S4 lief | S4 URLs/s | S0 URLs/s | Veränderung | S4 Writer s | S0 Writer s | S4 ohne Writer s | S0 ohne Writer s | S4 RSS MB | S0 RSS MB |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| r4 | erster | 1821 | 1786 | +2 % | 120,0 | 122,7 | 17,3 | 17,3 | 427 | 470 |
| r5 | erster | 1657 | 1620 | +2 % | 133,2 | 137,0 | 17,6 | 17,3 | 417 | 473 |

### Speicherzaehler des DIAG-Blocks (Spitzenwerte, erster Lauf je Stand; MB bzw. Eintraege)

| Stand | Seiten | Seen | Warteschlange (RAM) | Sortierstruktur | Writer-Batch (max) | Kern gesamt | Eintraege RAM | Eintraege nur in Datei | nicht auslagerbar | ausgelagert/nachgeladen | Nachladevorgaenge |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|
| S0 Ausgangsstand | 100000 | 2,9 | 11,6 | 3,1 | 26,7 | 17,6 | 88560 | 0 | 0 | 0 / 0 | 0 |
| S1 Seen-Menge (256 Teiltabellen) | 100000 | 2,8 | 11,6 | 3,1 | 27,5 | 17,5 | 88611 | 0 | 0 | 0 / 0 | 0 |
| S2 + RAM-Fenster (256 Teiltabellen) | 100000 | 2,8 | 5,7 | 1,6 | 26,2 | 8,7 | 42194 | 75007 | 0 | 77471 / 77471 | 4 |
| S3 wie S2 mit 16 Teiltabellen | 100000 | 2,8 | 5,7 | 1,6 | 26,2 | 8,7 | 42241 | 74919 | 0 | 77465 / 77465 | 4 |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 100000 | 3,1 | 3,0 | 0,8 | 27,5 | 5,6 | 22862 | 77347 | 0 | 88471 / 88471 | 9 |
| S0 Ausgangsstand | 250000 | 11,5 | 24,4 | 6,3 | 27,1 | 42,2 | 207521 | 0 | 0 | 0 / 0 | 0 |
| S1 Seen-Menge (256 Teiltabellen) | 250000 | 9,9 | 24,4 | 6,3 | 26,5 | 40,4 | 207552 | 0 | 0 | 0 / 0 | 0 |
| S1b wie S1 mit 16 Teiltabellen | 250000 | 10,0 | 24,4 | 6,3 | 26,8 | 40,7 | 207594 | 0 | 0 | 0 / 0 | 0 |
| S2 + RAM-Fenster (256 Teiltabellen) | 250000 | 9,9 | 5,8 | 1,6 | 27,1 | 15,9 | 42340 | 193640 | 0 | 227528 / 227528 | 12 |
| S3 wie S2 mit 16 Teiltabellen | 250000 | 10,0 | 8,9 | 1,6 | 26,3 | 19,6 | 42402 | 193344 | 0 | 227534 / 227534 | 12 |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 250000 | 6,3 | 3,0 | 0,8 | 28,4 | 9,2 | 22996 | 201700 | 0 | 238478 / 238478 | 24 |
| S0 Ausgangsstand | 500000 | 22,9 | 49,3 | 12,6 | 28,6 | 84,8 | 423098 | 0 | 0 | 0 / 0 | 0 |
| S1 Seen-Menge (256 Teiltabellen) | 500000 | 19,9 | 49,2 | 12,6 | 29,0 | 81,5 | 423107 | 0 | 0 | 0 / 0 | 0 |
| S2 + RAM-Fenster (256 Teiltabellen) | 500000 | 19,9 | 5,8 | 1,6 | 27,4 | 25,9 | 42389 | 410085 | 0 | 477530 / 477530 | 24 |
| S3 wie S2 mit 16 Teiltabellen | 500000 | 19,9 | 5,7 | 1,6 | 27,8 | 25,9 | 42319 | 410161 | 0 | 477529 / 477529 | 24 |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 500000 | 12,6 | 4,5 | 0,8 | 28,3 | 17,5 | 23048 | 417432 | 0 | 488477 / 488477 | 49 |

## Aufschlüsselung des Zuwachses je 100 000 URLs


| Stand | Seiten | Spitzen-RSS | Kernstrukturen (Seen, Warteschlange, Sortierstruktur, Weiterleitungen) | Writer-Batch (max) | Rest (SQLite-Cache 256 MiB, Bibliotheken, Allokator, Fragmentierung) |
|---|---:|---:|---:|---:|---:|
| S0 Ausgangsstand | 100000 | 424 | 17,6 | 26,7 | 380 |
| S0 Ausgangsstand | 250000 | 471 | 42,2 | 27,1 | 401 |
| S0 Ausgangsstand | 500000 | 538 | 84,8 | 28,6 | 424 |
| S1 Seen-Menge (256 Teiltabellen) | 100000 | 425 | 17,5 | 27,5 | 380 |
| S1 Seen-Menge (256 Teiltabellen) | 250000 | 622 | 40,4 | 26,5 | 556 |
| S1 Seen-Menge (256 Teiltabellen) | 500000 | 701 | 81,5 | 29,0 | 591 |
| S1b wie S1 mit 16 Teiltabellen | 250000 | 455 | 40,7 | 26,8 | 388 |
| S2 + RAM-Fenster (256 Teiltabellen) | 100000 | 412 | 8,7 | 26,2 | 377 |
| S2 + RAM-Fenster (256 Teiltabellen) | 250000 | 465 | 15,9 | 27,1 | 422 |
| S2 + RAM-Fenster (256 Teiltabellen) | 500000 | 460 | 25,9 | 27,4 | 406 |
| S3 wie S2 mit 16 Teiltabellen | 100000 | 412 | 8,7 | 26,2 | 377 |
| S3 wie S2 mit 16 Teiltabellen | 250000 | 427 | 19,6 | 26,3 | 381 |
| S3 wie S2 mit 16 Teiltabellen | 500000 | 446 | 25,9 | 27,8 | 392 |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 100000 | 408 | 5,6 | 27,5 | 375 |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 250000 | 420 | 9,2 | 28,4 | 382 |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 500000 | 433 | 17,5 | 28,3 | 387 |

Zuwachs je 100 000 Seiten zwischen 250 000 und 500 000 Seiten (MB):

| Stand | Spitzen-RSS | Kernstrukturen | Rest |
|---|---:|---:|---:|
| S0 Ausgangsstand | 26,8 | 17,0 | 9,2 |
| S1 Seen-Menge (256 Teiltabellen) | 31,5 | 16,4 | 14,1 |
| S2 + RAM-Fenster (256 Teiltabellen) | -2,2 | 4,0 | -6,3 |
| S3 wie S2 mit 16 Teiltabellen | 7,7 | 2,5 | 4,6 |
| S4 Endstand (kompakte Seen-Tabelle, Fenster 10 000) | 5,2 | 3,3 | 1,9 |

## Befunde

### 1. Die Posten der Engine sind jetzt flach; der Rest steht außerhalb von `os-engine`

- Die vom Pfad der Engine abhängigen Strukturen (Seen-Menge, Warteschlange, Sortierstruktur, Weiterleitungen) wuchsen im Ausgangsstand um 17 MB je 100 000 URLs auf 85 MB bei 500 000 Seiten (Hochrechnung 1 Mio.: 170 MB). Im Endstand wachsen nur noch die Seen-Menge (rund 2,4 MB je 100 000 URLs, 24 MB bei 1 Mio.); Warteschlange und Sortierstruktur bleiben bei rund 5 MB, die Bitmengen für die ausgelagerten Kennungen kosten 2 × 128 KB je 1 Mio. URLs.
- Der verbleibende Spitzen-RSS besteht zu rund 375 bis 425 MB aus dem „Rest“: SQLite-Seitencache (256 MiB, CRAWL-124, fest), die eingeblendeten System- und Programmbibliotheken (`vmmap`: rund 150 MB Text und Daten, die als resident zählen), Allokator und Fragmentierung. Der Rest wächst mit der Seitenzahl um weniger als 10 MB je 100 000 Seiten (S0: 9, S4: 3) und streut zwischen Läufen um ±20 MB; diese Streuung hängt am Schreibpfad (Größe der Commits, Zeitpunkt der Checkpoints) und liegt im Besitz von M1-P32 (`os-store`), nicht von M1-P33.
- Der Writer-Batch (`mem_writer_batch_bytes_max`, 26 bis 29 MB) ist durch `BATCH_PAGES_BACKLOG` begrenzt und unabhängig von der URL-Zahl; `engine_w_pages_max` (größter geschriebener Batch in Seiten, Schritt 4 des Pakets) steht in allen Berichten bei 2 000.

### 2. Befund: 256 Teiltabellen der Seen-Menge erhöhen den Spitzen-RSS um bis zu 160 MB (S1)

S1 (Seen-Menge in 256 `HashMap`-Teiltabellen, Zähler unauffällig) zeigte bei `perf-003` 250 000 Seiten 548 bis 628 MB gegen 468 bis 484 MB im Ausgangsstand und bei 500 000 Seiten 701 MB gegen 538 MB; der RSS-Verlauf (`s1-seen/rss-perf-003-500000.csv`) steigt dort nach dem ersten Höchstwert stetig weiter, während er im Ausgangsstand flach bleibt. Dieselbe Seen-Menge mit 16 Teiltabellen (S1b) lag bei 454 und 456 MB. Ursache nach Messung: 256 Tabellen von wenigen zehn KB liegen im kleinen Bereich des Allokators (macOS), ihre freigegebenen Vorgänger fragmentieren ihn; große Tabellen gehen beim Vergrößern an das Betriebssystem zurück. Die Zähler des DIAG-Blocks sehen das nicht (sie schätzen Bytes der Strukturen, nicht die Fragmentierung). Konsequenz: Die Seen-Menge ist ein einziger großer Block (offene Adressierung, S4), nicht viele kleine.

### 3. Seen-Menge bei 1 Mio.

16 Byte je Eintrag in den nach Kennung indizierten Feldern (Schlüssel 8, Fingerabdruck 4, Tiefe und Flags 4) plus 4 Byte je Platz der Tabelle bei Belegung 3/8 bis 3/4: rund 24,5 MB bei 1 Mio. Einträgen (S3 mit `HashMap`-Teiltabellen: 40 MB, weil 1 Mio. kurz nach dem Verdoppeln bei 917 000 Einträgen liegt und die Tabellen nur zur Hälfte belegt sind; Ausgangsstand: 2^21 Plätze zu 25 Byte, rund 52 MB). Beim Vergrößern der Tabelle bestehen kurz die alte und die neue (4,2 + 8,4 MB); die Felder je Kennung wachsen in Schritten von 32 768 Einträgen und werden nicht verdoppelt.

### 4. Durchsatz

Der Durchsatz der Kurzproben ist Writer-gebunden und streut stark (siehe Tabellen, Spalte „davon Schreiben“). Die Zeit **ohne** den Writer („Rest“) liegt in den gepaarten Läufen bei 17 bis 18 s je 250 000 Seiten, bei S0 wie bei S4 (S4: 16,7 bis 17,6 s, Ausnahme 22,3 s in dem Lauf mit 348 s Writer-Zeit), und ändert sich durch die Engine-Änderungen nicht. Die Aufträge der Warteschlange kosten je Auftrag einen `flush` des Writers (der den offenen Batch früher schreibt) und eine Abfrage von höchstens W Zeilen: 49 Aufträge bei 500 000 Seiten, 24 bei 250 000 und 9 bei 100 000 (mit W = 10 000).

### 5. Pausieren und Fortsetzen mit 1 Mio. Warteschlangeneinträgen (CRAWL-126)

`lifecycle_pause` (`--ignored`, 1 Mio. eingeplante URLs, direkt in SQL erzeugt, Release-Build):

| Stand | Lauf | Fortsetzen (Aufbau aus der Datei) | Pausieren | Spitzen-RSS des Testprozesses MB |
|---|---|---:|---:|---:|
| S1 (alter Aufbau: ganze Warteschlange ins RAM) | 1 (blockG) | 1156 ms | 71 ms | 704 |
| S3 (RAM-Fenster 20 000) | 1 (blockG) | 711 ms | 8 ms | 414 |
| S1 (alter Aufbau: ganze Warteschlange ins RAM) | 2 (blockG) | 1148 ms | 70 ms | 704 |
| S3 (RAM-Fenster 20 000) | 2 (blockG) | 710 ms | 12 ms | 414 |
| S1 (alter Aufbau: ganze Warteschlange ins RAM) | 1 (blockG2) | 1159 ms | 75 ms | 704 |
| S4 (RAM-Fenster 10 000) | 1 (blockG2) | 663 ms | 10 ms | 388 |
| S1 (alter Aufbau: ganze Warteschlange ins RAM) | 2 (blockG2) | 1134 ms | 71 ms | 704 |
| S4 (RAM-Fenster 10 000) | 2 (blockG2) | 668 ms | 11 ms | 387 |

Die gemessene Zeit „Fortsetzen“ enthält den Lauf über alle 1 Mio. Zeilen von `urls` (Seen-Menge) und die Ressourcen-Erkennung; die Warteschlange selbst wird nur noch für das Fenster gelesen. Der Spitzen-RSS des Testprozesses (einschließlich der Erzeugung der Datei) sinkt von 704 auf 388 MB (S3: 414 MB).

### 6. PERF-005 (K-49, harte Abbrüche) mit Fenster

`os-loadtest run --scenario perf-005 --size large --pages 100000` (5 gesäte harte Abbrüche zwischen 20 000 und 70 000 abgerufenen Seiten, 8 Threads; die Warteschlange ist bei jedem Abbruch größer als das Fenster, das Fortsetzen lädt also nur das Fenster und lädt den Rest nach):

| Stand | Abbrüche | wiederholte Requests je Abbruch (Mittel) | Dubletten in `urls` | fehlend / zusätzlich gegen den Referenzcrawl | Status je URL identisch | `integrity_check` ok | Dauer s |
|---|---:|---:|---:|---|---|---|---:|
| S2 (`de626ba`, Fenster 20 000, 256 Teiltabellen) | 5 | 1889 (Grenze 2264) | 0 | 0 / 0 | ja | ja | 58 |
| S4 (Endstand) | 5 | 1703 (Grenze 2264) | 0 | 0 / 0 | ja | ja | 47 |

Das Beispielprogramm `crawl_child` aus dem jeweiligen Stand wird von `perf-005` neben der Binärdatei gesucht; deshalb liefen diese Läufe mit der Binärdatei aus dem Build-Ordner des Pakets (`s2-example/`, `s4-example/`).

### 7. Was nicht gemessen wurde

- Der Lauf bei 1 Mio. Seiten (M1-N21). Die Hochrechnung ist eine lineare Fortschreibung der Spitzenwerte von 250 000 auf 500 000 Seiten; wegen der Streuung der Läufe (±20 MB) hat sie eine Unsicherheit in der gleichen Größe. Die Strukturen der Engine wurden bei 1 Mio. nur gerechnet (siehe Befund 3), nicht gemessen.
- Der Einfluss von M1-P32 (Linkindex erst am Ende, Zählertabelle, Hintergrund-Checkpoints). Er verändert den Anteil „Rest“ und die Streuung; die Messung dieses Pakets läuft auf `origin/main` ohne P32.

## Abweichungen von der Vorgabe und Entscheidungen

- **Schritte 2 und 3 in einem Commit** (`de626ba`): Das Fenster der Warteschlange und der Aufbau beim Fortsetzen hängen an denselben Dateien (`queue.rs`, `queue_disk.rs`, `resume.rs`); die Messung nach Schritt 2 gilt für Schritt 3 unverändert (kein Einfluss auf Läufe ohne Fortsetzen).
- **Spätere Anpassungen nach Messung** (`a6c386b`, `6080f7f`): Seen-Menge zuerst mit 16 statt 256 Teiltabellen (Befund 2), dann als einzelne Tabelle mit offener Adressierung (24 statt 40 MB bei 1 Mio.); Fenster W = 10 000 statt 20 000 (K-78 nennt „Größenordnung 20 000“; gleiche Größenordnung, Begründung an der Konstante `QUEUE_WINDOW`).
- **Wohin die Einträge gehen:** Die Entdeckung sendet den Batch jetzt **unter der Kern-Sperre** in den Kanal (`worker.rs`: `Slot::send` wartet nie; ebenso beim Sitemap-Ingest). Das Fenster der Warteschlange verlässt sich darauf, dass jeder eingeplante Eintrag nach einem `flush` des Writers in der Datei steht; ohne diese Ordnung könnte ein Eintrag zwischen dem Einplanen und dem Senden aus dem Fenster fallen. Das ändert die Reihenfolge der Nachrichten im Kanal nicht (Entdeckung vor Seite bleibt).
- **`Pop::NeedRefill`** ist eine neue Variante von `frontier::Pop` (additive Änderung der öffentlichen Schnittstelle; einziger Aufrufer ist die Steuerschleife). `Frontier` wohnt jetzt in `queue.rs` und wird aus `frontier.rs` wieder exportiert; `Frontier::improve` bleibt, `improve_with` kommt hinzu.
- **Ressourcen-Erkennung beim Fortsetzen:** Die Linktypen für „Ressource“ umfassen jetzt auch `Other` (wie `resource_like` im Worker); vorher wurden so gefundene Ziele nach dem Fortsetzen als Seite abgerufen. Kein Golden betroffen.
- **Test `queued_resources_are_still_fetched_as_resources…`:** Die Fixture `crawl-resources` enthält ein externes `https://cdn.other.test/…`, dessen Abruf gegen den (nur HTTP sprechenden) Testserver erst in die Zeitgrenze des Verbindungsaufbaus (10 s) läuft; der Test prüft deshalb ohne Prüfung externer Ziele und drosselt die erste Phase (`max_urls_per_sec`), damit die Pause vor dem Ende greift.
- **Nicht umgesetzt / Rest:** `docs/ARCHITECTURE.md` (Besitz M1-P32) ist nicht angepasst; die Beschreibung des Fensters steht in den Moduldokumentationen von `queue.rs`, `queue_disk.rs`, `queue_maint.rs`, `seen.rs`.
