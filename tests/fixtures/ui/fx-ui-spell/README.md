# Rechtschreib- und Grammatik-Fixture (M2-D19)

Szenario `ui/fx-ui-spell`, Start `/index.html` (verlinkt alle Seiten und `fehler.pdf` aus dem `<nav>`, also außerhalb des
Inhaltsbereichs). Gedacht für ONPAGE-153 … ONPAGE-157, ONPAGE-166 und den Detail-Tab UI-047.

- `dictionaries/` enthält **eigene Mini-Wortlisten** (Hunspell-Format, ohne Affixe außer `de.aff`) für die Tests;
  sie sind keine echten Wörterbücher. Englisch prüfen die Tests gegen die gebündelten SCOWL-Wörterbücher von
  `os-parse`. Die Tests setzen `dictionaries/` als Wörterbuchverzeichnis (`os_parse::spelling::dictionary::configure`).
- `langs/<Code>.html`: Smoke-Seite je Sprache der Stufen 1 und 2 mit einem korrekten und einem falschen Wort.
- `de-errors.html`: „Kontackt“ im Titel, „Beschreibng“ in der Meta-Description, „Rechtschreibfeler“ im `<main>`.
- `grammar-de.html`: die deutschen Beispielsätze aus ONPAGE-154 je in einem `<p>` (7 Grammatikfehler).
- `out-052.html`, `summary-1.html`, `summary-2.html`: „Beispeil“ auf 3 Seiten, insgesamt 5-mal (OUT-025/OUT-052).
- `fehler.pdf` (aus `generate_pdf.py`): ein Fehlerwort im Textlayer, `/Lang (de-DE)`, Titel „Test“.
- `expected/de-errors.json`: erwartete Zeilen des Detail-Tabs für `de-errors.html` (Spalten aus ONPAGE-157).

Die Tests nutzen die Spec-Defaults der Sprachwahl (`en` → `en-GB`, globale Default-Sprache `en-GB`), siehe
`docs/plan/M2-reports/d19.md` (offene Frage zu den Defaults in `os-model`).
