# Directives: Noindex

Typ: Warning (T-W1) · Priorität: High (P-H1)

## Deutsch

**Was bedeutet das?** Die Seite trägt eine noindex-Anweisung im Meta-Robots-Tag oder im X-Robots-Tag-Header.

**Warum ist es relevant?** Noindex hält die Seite aus den Suchergebnissen heraus. Bei internen oder dünnen Seiten ist das gewollt, versehentlich gesetzt macht es eine wichtige Seite unsichtbar.

**Wie beheben?** Prüfen, ob die Anweisung gewollt ist. Soll die Seite in der Suche erscheinen, noindex in der Vorlage, der Plugin-Einstellung oder dem Server-Header entfernen und zur Bestätigung neu crawlen.

## English

**What does it mean?** The page carries a noindex directive in the meta robots tag or the X-Robots-Tag header.

**Why does it matter?** Noindex keeps the page out of search results. That is intended for internal or thin pages, but set by mistake it makes an important page invisible.

**How to fix** Check whether the directive is intended. If the page should appear in search, remove the noindex directive from the template, plugin setting or server header, and recrawl to confirm.
