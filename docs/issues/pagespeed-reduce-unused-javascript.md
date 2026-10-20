# PageSpeed: Reduce Unused JavaScript

Typ: Opportunity (T-O2) · Priorität: Medium (P-M1)

## Deutsch

**Was bedeutet das?** Ein großer Teil des geladenen JavaScripts wird beim Laden der Seite nicht ausgeführt.

**Warum ist es relevant?** Ungenutzter Code kostet Download-Bytes sowie Zeit für Parsen und Kompilieren.

**Wie beheben?** Teile Bundles auf (Code-Splitting), lade Funktionen erst bei Bedarf und entferne nicht benötigte Bibliotheken.

## English

**What does it mean?** A large share of the loaded JavaScript is not executed while the page loads.

**Why does it matter?** Unused code costs download bytes and parse and compile time.

**How to fix** Split bundles (code splitting), load features on demand and remove libraries you do not need.
