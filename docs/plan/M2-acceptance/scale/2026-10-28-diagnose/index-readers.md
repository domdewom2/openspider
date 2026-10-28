# Korrektur der Indexpause

Der erste neue 250k-Diagnoselauf wurde nach 116.766 URLs abgebrochen: anfänglich ca. 2.450 URLs/s (56.761 URLs/24 s), danach ca. 400 URLs/s laut Fortschritt. Ein zweisekündiges macOS-Prozessprofil zeigte 1.354 von 1.384 Writer-Samples im booleschen `query_row` von `write_page`.

`writer.rs::write_page` fragt bei Status-/Indexierbarkeitsänderungen `EXISTS` auf `canonicals.target_id`, `pagination.target_id` und `hreflang.target_id` ab; `stamp_dependants` liest dieselben Ziele. Diese drei Indizes dürfen deshalb nicht ausgesetzt werden. Ihre Pause aus c8088a06 ist zurückgenommen; der Regressionstest prüft ihre Anwesenheit während des Crawls. `url_counters_change_seq` bleibt für den laufenden Änderungsfeed erhalten. `links_target` war bereits vor E07 ausgesetzt und wird am Ende sortiert aufgebaut. Nur `sitemap_entries_url` kommt zusätzlich in die Pause.

Der abgebrochene Lauf ist keine Abnahmemessung. Seine Crawl-Datei wurde gelöscht. Ein verwaister Lauf aus der vorigen Session wurde ebenfalls beendet; seine Überschneidung mit dem Build macht ihn als Vergleich unbrauchbar.
