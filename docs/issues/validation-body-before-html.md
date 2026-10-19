# Validation: <body> Element Preceding <html>

Typ: Warning (T-W2) · Priorität: High (P-H2)

## Deutsch

**Was bedeutet das?** Noch vor dem ersten html-Tag steht ein Element des Seiteninhalts oder sichtbarer Text.

**Warum ist es relevant?** Der Parser beginnt den Body vorzeitig; der spätere Head und seine Metadaten können ihre Wirkung verlieren.

**Wie beheben?** Entferne vorangestellte Inhalte aus der Ausgabe und setze sichtbare Inhalte in den Body hinter den Head.

## English

**What does it mean?** Page content or visible text appears before the first html tag.

**Why does it matter?** The parser enters the body early; the later head and its metadata may no longer take effect.

**How to fix** Remove leading output and place visible page content inside the body after the document head.
