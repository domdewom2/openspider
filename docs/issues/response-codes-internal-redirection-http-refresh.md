# Response Codes: Internal Redirection (HTTP Refresh)

Typ: Warning (T-W2) · Priorität: Low (P-L1)

## Deutsch

**Was bedeutet das?** Der Antwortheader verweist Besucher per Refresh auf eine andere URL.

**Warum ist es relevant?** Eine verzögerte Navigation verlängert die Wartezeit und kann von Crawlern unterschiedlich behandelt werden. Links auf diese Adresse verursachen einen zusätzlichen Navigationsschritt.

**Wie beheben?** Verlinke direkt auf das Ziel. Ersetze den Refresh-Header bei einem dauerhaften Umzug durch eine passende HTTP-Weiterleitung.

## English

**What does it mean?** The response header sends visitors to another URL using Refresh.

**Why does it matter?** A delayed navigation adds waiting time and can be handled differently by crawlers. Links to this address require an additional navigation.

**How to fix** Link directly to the destination. If the move is permanent, replace the Refresh header with an appropriate HTTP redirect.
