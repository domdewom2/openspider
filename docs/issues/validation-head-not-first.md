# Validation: <head> Not First In <html> Element

Typ: Warning (T-W2) · Priorität: High (P-H2)

## Deutsch

**Was bedeutet das?** Vor dem ersten Head stehen bereits Elemente oder Text im Dokument.

**Warum ist es relevant?** Der Parser kann vorzeitig zum Body wechseln, sodass spätere Head-Metadaten nicht mehr wirksam sind.

**Wie beheben?** Setze den Head direkt hinter das öffnende html-Tag. Verschiebe sichtbare Inhalte hinter den Head in den Body.

## English

**What does it mean?** Elements or text appear before the first head in the document.

**Why does it matter?** The parser may enter the body early, preventing later head metadata from taking effect.

**How to fix** Place the head immediately after the opening html tag. Move visible contents after the head into the body.
