# Security: Unsafe Cross-Origin Links

Typ: Warning (T-W3) · Priorität: Low (P-L2)

## Deutsch

**Was bedeutet das?** Ein externer Link öffnet ein neues Fenster ohne explizites noopener oder noreferrer.

**Warum ist es relevant?** Moderne Browser setzen noopener bei _blank implizit. Ältere Browser können dem Ziel Zugriff auf das Ursprungsfenster erlauben.

**Wie beheben?** Bei externen Links mit target=_blank das rel-Token noopener oder noreferrer ergänzen.

## English

**What does it mean?** An external link opens a new window without explicit noopener or noreferrer.

**Why does it matter?** Modern browsers imply noopener for _blank. Older browsers may let the destination access the opener window.

**How to fix** Add the noopener or noreferrer rel token to external links with target=_blank.
