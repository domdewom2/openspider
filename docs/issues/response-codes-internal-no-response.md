# Response Codes: Internal No Response

Typ: Issue (T-I1) · Priorität: High (P-H1)

## Deutsch

**Was bedeutet das?** Der Server hat für diese interne URL nicht geantwortet, etwa wegen einer Zeitüberschreitung, einer abgelehnten Verbindung oder einer fehlgeschlagenen DNS-Auflösung.

**Warum ist es relevant?** Besucher und Crawler erreichen die Seite überhaupt nicht. Wiederholte Ausfälle führen dazu, dass die URL aus dem Index fällt, und sie verbrauchen Crawl-Kapazität.

**Wie beheben?** Den zur URL vermerkten Grund prüfen, etwa Zeitüberschreitung oder Verbindungsfehler. DNS-Einträge, Erreichbarkeit des Servers und Firewall-Regeln kontrollieren und sicherstellen, dass die Adresse in den verweisenden Links richtig geschrieben ist. Bei überlastetem Server Kapazität erhöhen.

## English

**What does it mean?** The server did not answer for this internal URL, for example because of a timeout, a refused connection or a failed DNS lookup.

**Why does it matter?** Visitors and crawlers cannot reach the page at all. Repeated failures lead to the URL being dropped from the index and waste crawl capacity.

**How to fix** Check the reason recorded for the URL, such as timeout or connection error. Verify DNS records, server availability and firewall rules, and make sure the address is spelled correctly in the links that point to it. Add capacity if the host is overloaded.
