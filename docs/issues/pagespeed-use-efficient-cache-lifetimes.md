# PageSpeed: Use Efficient Cache Lifetimes

Typ: Opportunity (T-O2) · Priorität: Medium (P-M1)

## Deutsch

**Was bedeutet das?** Statische Ressourcen werden ohne oder mit zu kurzer Cache-Lebensdauer ausgeliefert.

**Warum ist es relevant?** Wiederkehrende Besucher laden dieselben Dateien erneut; das kostet Bytes und Zeit.

**Wie beheben?** Setze für versionierte statische Dateien lange Cache-Lebensdauern (Cache-Control max-age).

## English

**What does it mean?** Static resources are served without a cache lifetime or with one that is too short.

**Why does it matter?** Returning visitors download the same files again, which costs bytes and time.

**How to fix** Give versioned static files long cache lifetimes (Cache-Control max-age).
