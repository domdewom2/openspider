# Issue-Katalog

Je Eintrag des Issue-Katalogs (`core/issues/catalog.toml`) gibt es hier eine Seite `<doc_key>.md` mit
Klassifizierung (Typ, Priorität und Regelcodes nach UI-063) sowie den drei Texten *Was bedeutet das?*,
*Warum ist es relevant?* und *Wie beheben?* in Deutsch und Englisch.

Klassifizierung abgeleitet aus öffentlicher Dokumentation der Referenz (Benchmark), begründet nach UI-063.

Die Seiten werden aus Katalog und `core/issues/texts/{de,en}.toml` erzeugt:

```sh
UPDATE_GOLDEN=1 cargo test -p os-analyze --test issues_texts doc_pages
```

Die Texte sind eigenständig für OpenSpider formuliert (Clean-Room). Änderungen an Texten werden ohne Einsicht in
Erklärungstexte der Referenz verfasst.
