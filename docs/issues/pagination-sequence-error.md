# Pagination: Sequence Error

Typ: Issue (T-I3) · Priorität: Low (P-L1)

Befüllt erst nach der Crawl-Analyse.

## Deutsch

**Was bedeutet das?** Eine Paginierungsangabe der Seite wird vom abgerufenen HTML-200-Ziel nicht bestätigt.

**Warum ist es relevant?** Wenn next und prev nicht wechselseitig sind, beschreiben die Seiten widersprüchliche Reihenfolgen derselben Serie.

**Wie beheben?** Gleiche die Verweise ab: auf next von A nach B muss prev von B nach A folgen; prüfe auch die Gegenrichtung.

## English

**What does it mean?** A pagination annotation on this page is not confirmed by the crawled HTML target returning status 200.

**Why does it matter?** When next and prev are not reciprocal, the pages describe conflicting orders for the same series.

**How to fix** Reconcile both directions: next from A to B needs prev from B to A, and prev from A to B needs next from B to A.
