# Response Codes: Internal Blocked by Robots.txt

Typ: Warning (T-W1) · Priorität: High (P-H1)

## Deutsch

**Was bedeutet das?** Die robots.txt der Website verbietet das Crawlen dieser internen URL, ihr Inhalt wurde daher nicht abgerufen.

**Warum ist es relevant?** Gesperrte Seiten können von Crawlern nicht gelesen werden. Sie können trotzdem als nackte Adressen ohne Beschreibung in den Suchergebnissen auftauchen, und Links auf sie geben keinen lesbaren Zusammenhang weiter.

**Wie beheben?** Prüfen, ob die Sperre gewollt ist. Soll die Seite gefunden werden, die passende Disallow-Regel in der robots.txt entfernen oder enger fassen. Soll sie nicht in den Suchergebnissen erscheinen, das Crawlen erlauben und stattdessen noindex setzen.

## English

**What does it mean?** The robots.txt file of the site forbids crawling this internal URL, so its content was not fetched.

**Why does it matter?** Blocked pages cannot be read by crawlers. They can still show up in search results as bare addresses without a description, and links pointing to them pass on no readable context.

**How to fix** Check whether the block is intended. If the page should be found, remove or narrow the matching Disallow rule in robots.txt. If it should stay out of search results, allow crawling and use a noindex directive instead.
