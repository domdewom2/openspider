# PageSpeed: LCP Request Discovery

Typ: Opportunity (T-O2) · Priorität: Medium (P-M1)

## Deutsch

**Was bedeutet das?** Die Ressource des größten sichtbaren Elements (LCP) wird spät entdeckt oder nachrangig geladen, etwa per Lazy-Loading oder ohne hohe Ladepriorität.

**Warum ist es relevant?** Der Browser kann das wichtigste Bild erst spät anfordern; der Largest Contentful Paint verschlechtert sich.

**Wie beheben?** Binde das LCP-Bild direkt im HTML ein, verzichte dort auf Lazy-Loading und setze fetchpriority="high".

## English

**What does it mean?** The resource of the largest visible element (LCP) is discovered late or loaded with low priority, for example lazily or without a high fetch priority.

**Why does it matter?** The browser can request the most important image only late, which worsens Largest Contentful Paint.

**How to fix** Reference the LCP image directly in the HTML, do not lazy-load it and add fetchpriority="high".
