# Abschlussphasen und WAL-Ursache (250k)

Release-Diagnose nach 36c33508 mit noch uncommittierter additiver Phaseninstrumentierung; vollständige Messdaten in stage-250000.json. Start-Load < 2, caffeinate, keine parallelen Builds/Tests. Crawl-Datei anschließend gelöscht.

| Phase | Dauer ms | WAL vorher Bytes | WAL nachher Bytes |
|---|---:|---:|---:|
| Indexaufbau | 3.754 | 513.269.632 | 515.947.632 |
| Endbatch insgesamt | 4.733 | 513.269.632 | 519.618.552 |
| Checkpoint | 84 | 519.618.552 | 0 |
| Canonicals | 315 | 57.712 | 57.712 |
| Pagination | 84 | 57.712 | 57.712 |
| Hreflang | 28 | 57.712 | 57.712 |
| Links | 31.147 | 57.712 | 57.712 |
| LinkScore | 61.763 | 57.712 | 591.747.392 |

Die übrigen protokollierten Analysatoren benötigen < 1 ms. Analysatoren laufen parallel: ihre Dauern sind nicht addierbar; das WAL ist gemeinsam. Die Links-Analyse endet ohne WAL-Zuwachs, der große Zuwachs entsteht beim späteren Abschluss von LinkScore. Dies bestimmt die Ursache der Spitze im neuen Lauf: die atomare Veröffentlichung der Link-Score-Fakten samt Änderungsstempeln, nicht die Crawl-Checkpoint-Schwelle. Eine exakte rückwirkende Zuordnung der alten 3,4-GB-Spitze ist dadurch nicht möglich; der vorherige Lauf hatte noch eine andere Speicherform.

Zusätzliche 1k-Fixture zur Erklärung der Fragmentierung: content.text 969–1096 Zeichen, content.soft404_text 1002–1133 Zeichen. url_ext_data besitzt 1991 Overflow-Seiten mit 8.155.136 Bytes, davon 6.995.544 ungenutzt. Der maximale lokale Payload eines WITHOUT-ROWID-Baums ist bei 4-KiB-Seiten wesentlich kleiner als bei einem Rowid-Baum. Deshalb liegen diese knapp über 1 KiB langen Werte auf je einer fast leeren Overflow-Seite. DELETE+INSERT allein erklärt die Speicherlücke nicht.

Korrektur: lange Texte (> 768 UTF-8-Bytes) separat im Rowid-Baum ext_texts, kurze skalare Werte weiter im kompakten WITHOUT-ROWID-Baum; gleiche lesbare url_ext-Spalten über LEFT JOIN. Das senkt auch die Zahl der durch LinkScore berührten Seiten. Migration 0005 wird vor ihrer Veröffentlichung auf diesem Paketbranch vervollständigt (Version 5 bleibt wie freigegeben; keine bereits veröffentlichte Migration wird geändert). TRUNCATE-Checkpoints nach großen Analysephasen und nach Ende aller Arbeiter begrenzen den über Phasengrenzen gehaltenen WAL-Raum. Die atomare Ergebnisveröffentlichung bleibt erhalten.
