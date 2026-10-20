# PageSpeed: Legacy JavaScript

Typ: Opportunity (T-O2) · Priorität: Medium (P-M1)

## Deutsch

**Was bedeutet das?** Moderne Browser erhalten Polyfills und umgewandelten Code, den sie nicht brauchen.

**Warum ist es relevant?** Überflüssiger Code erhöht Download und Ausführungszeit.

**Wie beheben?** Erzeuge ein separates Bundle für moderne Browser und liefere Polyfills nur bei Bedarf aus.

## English

**What does it mean?** Modern browsers receive polyfills and transpiled code they do not need.

**Why does it matter?** Unneeded code increases download size and execution time.

**How to fix** Build a separate bundle for modern browsers and ship polyfills only where needed.
