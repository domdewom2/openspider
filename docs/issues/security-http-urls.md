# Security: HTTP URLs

Typ: Issue (T-I4) · Priorität: High (P-H3)

## Deutsch

**Was bedeutet das?** Die URL wird über unverschlüsseltes HTTP statt über HTTPS ausgeliefert.

**Warum ist es relevant?** Browser kennzeichnen unverschlüsselte Seiten als nicht sicher, jeder auf dem Übertragungsweg kann den Datenverkehr mitlesen oder verändern, und Suchmaschinen bevorzugen die verschlüsselte Fassung einer Seite.

**Wie beheben?** Ein gültiges Zertifikat einrichten, die gesamte Website über HTTPS ausliefern und jede HTTP-Adresse per dauerhafter Weiterleitung auf ihr HTTPS-Gegenstück schicken. Danach interne Links, Canonical-Angaben und Sitemaps auf die HTTPS-Adressen umstellen.

## English

**What does it mean?** The URL is delivered over plain, unencrypted HTTP instead of HTTPS.

**Why does it matter?** Browsers label unencrypted pages as not secure, anyone on the network path can read or alter the traffic, and search engines prefer the encrypted version of a page.

**How to fix** Install a valid certificate, serve the whole site over HTTPS and send every HTTP address to its HTTPS counterpart with a permanent redirect. Then update internal links, canonical elements and sitemaps to the HTTPS addresses.
