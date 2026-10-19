# Validation: Missing <body> Tag

Typ: Issue (T-I2) · Priorität: High (P-H2)

## Deutsch

**Was bedeutet das?** Im Originalquelltext fehlt ein öffnendes Body-Tag.

**Warum ist es relevant?** Der Parser muss die Inhaltsstruktur ergänzen. Bei älteren Frameset-Seiten bleibt die Struktur uneindeutig.

**Wie beheben?** Erzeuge einen expliziten Body für die Seiteninhalte und ersetze veraltete Frameset-Vorlagen durch gültiges HTML.

## English

**What does it mean?** The original source does not contain an opening body tag.

**Why does it matter?** The parser must infer the content structure. Older frameset documents also lack an explicit body.

**How to fix** Generate an explicit body for page content and replace obsolete frameset templates with valid HTML documents.
