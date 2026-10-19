# Pagination: Unlinked Pagination URLs

Typ: Issue (T-I3) · Priorität: Medium (P-M2)

Befüllt erst nach der Crawl-Analyse.

## Deutsch

**Was bedeutet das?** Mindestens ein abgerufenes Paginierungsziel hat keinen internen Hyperlink im Crawl.

**Warum ist es relevant?** Ohne interne Navigation ist die Folgeseite nur über die Paginierungsangabe auffindbar; der normale Linkfluss fehlt.

**Wie beheben?** Verlinke die Zielseite aus der internen Navigation und führe danach die Paginierungsanalyse erneut aus.

## English

**What does it mean?** At least one crawled pagination target has no hyperlink from any internal page in this crawl.

**Why does it matter?** Without internal navigation the target is discoverable only through the pagination annotation and receives no ordinary link flow.

**How to fix** Link to the target from internal navigation, then run the pagination analysis again to check the result.
