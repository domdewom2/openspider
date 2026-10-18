# Security: Missing Secure Referrer-Policy Header

Typ: Warning (T-W2) · Priorität: Low (P-L1)

## Deutsch

**Was bedeutet das?** Der Antwort fehlt ein gültiger Header Referrer-Policy.

**Warum ist es relevant?** Eine unsichere Richtlinie kann mehr Herkunftsinformationen als beabsichtigt weitergeben.

**Wie beheben?** Eine explizite sichere Referrer-Policy senden; in Listen zählt der letzte gültige Wert.

## English

**What does it mean?** The response has no secure effective Referrer-Policy value.

**Why does it matter?** An unsafe policy may disclose more source information than intended.

**How to fix** Send an explicit secure Referrer-Policy; the last recognized value in a list is effective.
