# PageSpeed: Forced Reflow

Typ: Opportunity (T-O2) · Priorität: Medium (P-M1)

## Deutsch

**Was bedeutet das?** JavaScript liest Layoutwerte unmittelbar nach Änderungen am DOM und zwingt den Browser zu einer sofortigen Neuberechnung.

**Warum ist es relevant?** Erzwungene Neuberechnungen blockieren den Hauptthread und machen die Seite träge.

**Wie beheben?** Bündele Lesezugriffe auf Layoutwerte vor den Schreibzugriffen und vermeide Lesen direkt nach DOM-Änderungen.

## English

**What does it mean?** JavaScript reads layout values right after changing the DOM and forces the browser to recalculate immediately.

**Why does it matter?** Forced recalculations block the main thread and make the page sluggish.

**How to fix** Batch reads of layout values before writes and avoid reading right after DOM changes.
