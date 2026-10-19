# Security: Missing Content-Security-Policy Header

Typ: Warning (T-W2) · Priorität: Low (P-L1)

## Deutsch

**Was bedeutet das?** Der Antwort fehlt ein gültiger Header Content-Security-Policy.

**Warum ist es relevant?** Ohne durchgesetzte CSP fehlt eine zusätzliche Begrenzung der erlaubten Ressourcen.

**Wie beheben?** Eine passende CSP als Antwort-Header einführen; Report-Only und Meta ersetzen diesen Header im Filter nicht.

## English

**What does it mean?** The response has no enforced Content-Security-Policy header.

**Why does it matter?** An enforced CSP adds restrictions on the resources a browser may load.

**How to fix** Introduce an appropriate response CSP; Report-Only and meta CSP do not satisfy this filter.
