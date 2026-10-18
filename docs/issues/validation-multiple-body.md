# Validation: Multiple <body> Tags

Typ: Issue (T-I2) · Priorität: High (P-H2)

## Deutsch

**Was bedeutet das?** Der Originalquelltext enthält mehrere öffnende Body-Tags.

**Warum ist es relevant?** Der Parser führt zusätzliche Body-Elemente zusammen oder verwirft sie; die beabsichtigte Struktur geht verloren.

**Wie beheben?** Erzeuge genau einen Body und entferne doppelte Wrapper aus eingebundenen Vorlagen oder Inhaltsfragmenten.

## English

**What does it mean?** The original source contains more than one opening body tag.

**Why does it matter?** The parser merges or discards additional body elements, losing the intended document structure.

**How to fix** Generate exactly one body and remove duplicate wrappers from included templates or content fragments.
