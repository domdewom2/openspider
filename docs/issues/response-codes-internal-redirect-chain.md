# Response Codes: Internal Redirect Chain

Typ: Warning (T-W2) · Priorität: Medium (P-M2)

## Deutsch

**Was bedeutet das?** Die Weiterleitungen dieser internen URL führen über mehrere Zwischenschritte, bevor die endgültige Adresse erreicht ist.

**Warum ist es relevant?** Jeder Schritt kostet Wartezeit, lange Ketten werden von Crawlern unter Umständen abgebrochen, bevor das Ziel erreicht ist, und Signale wie Linkwert können unterwegs verloren gehen.

**Wie beheben?** Die erste Adresse der Kette mit einer einzigen Weiterleitung direkt auf das Ziel schicken und interne Links auf die endgültige Adresse korrigieren. Die Weiterleitungsregeln prüfen, damit keine neuen Zwischenschritte hinzukommen.

## English

**What does it mean?** Following the redirects of this internal URL leads through several hops before the final address is reached.

**Why does it matter?** Each hop adds waiting time, long chains may be abandoned by crawlers before the target is reached, and signals such as link value can be lost along the way.

**How to fix** Send the first address of the chain straight to the final destination with a single redirect and correct internal links to use the final address. Review the redirect rules so that no new hops are stacked on top.
