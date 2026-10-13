# Response Codes: Internal Redirection (Meta Refresh)

Typ: Warning (T-W2) · Priorität: Low (P-L1)

## Deutsch

**Was bedeutet das?** Die Seite weist den Browser über eine Meta-Refresh-Anweisung im HTML an, nach dem Laden zu einer anderen Adresse zu wechseln.

**Warum ist es relevant?** Meta-Refresh-Weiterleitungen sind langsamer als Server-Weiterleitungen, können vor dem Wechsel kurz die alte Seite zeigen und werden von Suchmaschinen weniger zuverlässig ausgewertet als HTTP-Weiterleitungen.

**Wie beheben?** Den Meta-Refresh durch eine dauerhafte serverseitige Weiterleitung (HTTP 301 oder 308) von der alten auf die Zieladresse ersetzen und interne Links auf die Zieladresse umstellen.

## English

**What does it mean?** The page tells the browser to move on to another address after loading, using a meta refresh instruction in its HTML.

**Why does it matter?** Meta refresh redirects are slower than server redirects, can flash the old page before the switch and are treated less reliably by search engines than HTTP redirects.

**How to fix** Replace the meta refresh with a permanent server-side redirect (HTTP 301 or 308) from the old address to the target, and point internal links at the target address.
