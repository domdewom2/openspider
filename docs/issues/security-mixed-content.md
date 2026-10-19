# Security: Mixed Content

Typ: Issue (T-I4) · Priorität: High (P-H3)

## Deutsch

**Was bedeutet das?** Die HTTPS-Seite bindet mindestens eine Ressource über HTTP ein.

**Warum ist es relevant?** Browser können diese Ressource blockieren; unverschlüsselte Inhalte können unterwegs verändert werden.

**Wie beheben?** Alle betroffenen Ressourcen über HTTPS bereitstellen und ihre Verweise aktualisieren.

## English

**What does it mean?** The HTTPS page embeds at least one resource over HTTP.

**Why does it matter?** Browsers may block the resource, and unencrypted content can be altered in transit.

**How to fix** Serve every affected resource over HTTPS and update its references.
