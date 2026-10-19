# Pagination: Pagination Loop

Typ: Issue (T-I3) · Priorität: Low (P-L1)

Befüllt erst nach der Crawl-Analyse.

## Deutsch

**Was bedeutet das?** Die Seite liegt in einem Zyklus aus next-Verweisen oder aus prev-Verweisen.

**Warum ist es relevant?** Die Serie führt zu einer bereits besuchten Seite zurück und hat in dieser Richtung keinen eindeutigen Abschluss.

**Wie beheben?** Korrigiere die Verweise im Zyklus; der Anfang braucht kein prev und das Ende kein next. Starte die Analyse erneut.

## English

**What does it mean?** This page belongs to a cycle of next references or a cycle of prev references.

**Why does it matter?** The series returns to an already visited page and has no clear ending in that direction.

**How to fix** Correct the references within the cycle; omit prev at the start and next at the end, then run the analysis again.
