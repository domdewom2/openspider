# Security: Missing HSTS Header

Typ: Warning (T-W2) · Priorität: Low (P-L1)

## Deutsch

**Was bedeutet das?** Der Antwort fehlt ein gültiger Header Strict-Transport-Security.

**Warum ist es relevant?** Ohne HSTS erzwingt der Browser für spätere Besuche nicht automatisch HTTPS.

**Wie beheben?** Nach Prüfung der HTTPS-Abdeckung eine passende HSTS-Richtlinie senden.

## English

**What does it mean?** The HTTPS response has no Strict-Transport-Security header.

**Why does it matter?** Without HSTS, subsequent visits do not automatically require HTTPS.

**How to fix** After checking HTTPS coverage, send an appropriate HSTS policy.
