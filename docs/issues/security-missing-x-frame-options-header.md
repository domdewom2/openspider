# Security: Missing X-Frame-Options Header

Typ: Warning (T-W2) · Priorität: Low (P-L1)

## Deutsch

**Was bedeutet das?** Der Antwort fehlt ein gültiger Header X-Frame-Options: DENY oder SAMEORIGIN.

**Warum ist es relevant?** Ohne diesen Schutz kann die Seite unter Umständen in fremden Frames erscheinen.

**Wie beheben?** DENY oder SAMEORIGIN passend zur Einbettungsstrategie senden; CSP frame-ancestors ersetzt den Header für diesen Filter nicht.

## English

**What does it mean?** The response does not send X-Frame-Options with DENY or SAMEORIGIN.

**Why does it matter?** The page may be embedded in another site without this protection.

**How to fix** Send DENY or SAMEORIGIN as appropriate; CSP frame-ancestors does not replace this header for this filter.
