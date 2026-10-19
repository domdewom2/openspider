# Security: Missing X-Content-Type-Options Header

Typ: Warning (T-W2) · Priorität: Low (P-L1)

## Deutsch

**Was bedeutet das?** Der Antwort fehlt ein gültiger Header X-Content-Type-Options: nosniff.

**Warum ist es relevant?** Browser können Inhalte sonst unter einem unerwarteten Medientyp interpretieren.

**Wie beheben?** Den Antwort-Header X-Content-Type-Options mit dem Wert nosniff senden.

## English

**What does it mean?** The response does not send X-Content-Type-Options with the value nosniff.

**Why does it matter?** Browsers may otherwise interpret content using an unexpected media type.

**How to fix** Configure the server to send X-Content-Type-Options: nosniff on its responses.
