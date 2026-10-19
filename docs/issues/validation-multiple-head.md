# Validation: Multiple <head> Tags

Typ: Issue (T-I2) · Priorität: High (P-H2)

## Deutsch

**Was bedeutet das?** Der Originalquelltext enthält mehrere öffnende Head-Tags.

**Warum ist es relevant?** Zusätzliche Head-Tags werden vom Parser verworfen; nachfolgende Metadaten können dadurch unwirksam werden.

**Wie beheben?** Fasse alle Head-Metadaten in einem einzigen Head zusammen und korrigiere die erzeugenden Seitenvorlagen.

## English

**What does it mean?** The original source contains more than one opening head tag.

**Why does it matter?** Additional head tags are discarded by the parser and later metadata may no longer take effect.

**How to fix** Combine the metadata in a single head and correct the page templates that generate duplicate head tags.
