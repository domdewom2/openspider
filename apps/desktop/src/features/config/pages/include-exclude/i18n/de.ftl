# Seite Einschließen/Ausschließen (M2-U13)
config-ie-desc = Reguläre Ausdrücke, einer pro Zeile. Eine Regel muss die ganze URL treffen, sofern die Teilübereinstimmung nicht aktiv ist. Ausschließen wird vor Einschließen geprüft.
config-sec-include = Einschließen
config-sec-exclude = Ausschließen
config-sec-match = Abgleich
config-hint-include = Nur URLs, die mindestens eine Zeile treffen, werden gecrawlt. Leer = keine Einschränkung. Leere Zeilen zählen bei den Zeilennummern nicht mit.
config-hint-exclude = URLs, die irgendeine Zeile treffen, werden nicht gecrawlt. Leere Zeilen zählen bei den Zeilennummern nicht mit.
config-hint-partial = Auch Teile der URL abgleichen. Ohne diese Option den Rest mit .* abdecken.
config-f-scope-include = Einschlussregeln
config-f-scope-exclude = Ausschlussregeln
config-f-scope-regex-partial-match = Regeln treffen auch Teile der URL
config-ie-tester = Regex-Tester
config-ie-verdict-included = Eingeschlossen
config-ie-verdict-excluded = Ausgeschlossen
config-ie-verdict-notIncluded = Nicht eingeschlossen
config-ie-list-include = Einschließen
config-ie-list-exclude = Ausschließen
config-ie-line = { $list }-Zeile { $line }
config-ie-encoded = Geprüfte URL (so kodiert wie im Crawl)
config-ie-partial = { $line } passt nur teilweise – .* ergänzen?
config-ie-invalid = { $line } ist kein gültiger regulärer Ausdruck und wurde übersprungen.
