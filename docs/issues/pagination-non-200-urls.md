# Pagination: Non-200 Pagination URLs

Typ: Issue (T-I1) · Priorität: High (P-H1)

## Deutsch

**Was bedeutet das?** Mindestens ein abgerufenes Paginierungsziel antwortet nicht mit HTTP 200.

**Warum ist es relevant?** Weiterleitungen, Fehler und gesperrte Ziele unterbrechen den direkten Zugang zu weiteren Seiten der Serie.

**Wie beheben?** Prüfe Status und Erreichbarkeit des Ziels und verweise direkt auf eine erreichbare Seite mit Status 200.

## English

**What does it mean?** At least one crawled pagination target does not respond with HTTP status 200.

**Why does it matter?** Redirects, errors and blocked targets interrupt direct access to further pages in the series.

**How to fix** Check the target status and accessibility, then point directly to an available page returning status 200.
