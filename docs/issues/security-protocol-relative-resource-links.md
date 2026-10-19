# Security: Protocol-Relative Resource Links

Typ: Warning (T-W2) · Priorität: Low (P-L1)

## Deutsch

**Was bedeutet das?** Eine Ressourcenadresse beginnt mit // und übernimmt das Schema der Seite.

**Warum ist es relevant?** Auf HTTP-Seiten wird die Ressource dadurch ebenfalls unverschlüsselt geladen.

**Wie beheben?** Eine explizite HTTPS-Adresse verwenden und die Erreichbarkeit prüfen.

## English

**What does it mean?** A resource address starts with // and inherits the page scheme.

**Why does it matter?** On HTTP pages this also loads the resource without encryption.

**How to fix** Use an explicit HTTPS address and verify that the resource is available.
