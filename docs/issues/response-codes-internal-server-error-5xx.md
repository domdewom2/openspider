# Response Codes: Internal Server Error (5XX)

Typ: Issue (T-I1) · Priorität: High (P-H1)

## Deutsch

**Was bedeutet das?** Die interne URL antwortet mit einem Server-Fehler im Bereich 5xx, der Server konnte die Seite also nicht ausliefern.

**Warum ist es relevant?** Besucher sehen eine Fehlermeldung, und Crawler, die wiederholt Serverfehler erhalten, kommen seltener vorbei und nehmen die Seite eventuell aus dem Index, bis sie wieder funktioniert.

**Wie beheben?** In den Server-Protokollen zum Zeitpunkt der Anfrage die Ursache suchen, etwa ein fehlerhaftes Skript, ein Datenbankproblem oder einen überlasteten Server. Die Ursache beheben und die URL neu crawlen, um Status 200 zu bestätigen.

## English

**What does it mean?** The internal URL answers with a server error status in the 5xx range, so the server failed to deliver the page.

**Why does it matter?** Visitors see an error, and crawlers that meet repeated server errors visit less often and may remove the page from the index until it works again.

**How to fix** Look into the server logs for the time of the request to find the cause, for example a faulty script, a database problem or an overloaded host. Fix it and recrawl the URL to confirm that it answers with status 200.
