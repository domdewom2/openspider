# Response Codes: Internal Redirect Loop

Typ: Issue (T-I1) · Priorität: High (P-H1)

## Deutsch

**Was bedeutet das?** Die Weiterleitungen dieser internen URL führen zu einer bereits besuchten Adresse zurück und erreichen daher nie eine endgültige Seite.

**Warum ist es relevant?** Weder Browser noch Crawler können die Seite öffnen. Besucher sehen eine Fehlermeldung, und der Inhalt kann nicht indexiert werden.

**Wie beheben?** Die beteiligten Weiterleitungsregeln nachverfolgen, etwa widersprüchliche Regeln für www, abschließenden Schrägstrich oder HTTPS, und die Regel entfernen, die die Anfrage zurückschickt. Die korrigierte Adresse testen, bis sie auf einer Seite mit Status 200 endet.

## English

**What does it mean?** The redirects of this internal URL lead back to an address that was already visited, so they never reach a final page.

**Why does it matter?** Neither browsers nor crawlers can open the page. Visitors see an error and the content cannot be indexed.

**How to fix** Trace the redirect rules involved, for example conflicting rules for www, trailing slash or HTTPS, and remove the rule that sends the request back. Test the corrected address until it ends on a page with status 200.
