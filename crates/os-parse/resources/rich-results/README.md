# Regeldatei Google Rich Results (TECH-188)

`rules.json` beschreibt, welche Rich-Result-Features OpenSpider erkennt und welche Eigenschaften dafür Pflicht
(`required`, `one_of`, `required_top_level`, `required_unless_last`, auf AMP-Seiten zusätzlich `amp_required`) bzw.
empfohlen (`recommended`) sind. Die Datei ist eine eigene Zusammenstellung nach der öffentlichen Dokumentation von
Google Search Central; sie übernimmt keine Texte. Typ- und Eigenschaftsnamen sind schema.org-Bezeichner.

Felder je Feature:

| Feld | Bedeutung |
|---|---|
| `name` | Anzeigename, erscheint in `Rich Result Features` und in den Reports |
| `deprecated_since` | gesetzt = von Google eingestellt: weder erkannt noch validiert (OUT-102); die schema.org-Prüfung bleibt |
| `types` | auslösende Typen; Untertypen laut Vokabular lösen ebenfalls aus |
| `with` | zusätzlich nötige Eigenschaft zum Auslösen (z. B. `offers` für Merchant listing) |
| `nested_in` | verschachtelte Items lösen das Feature aus, wenn ihr direkter Elternknoten einen dieser Typen hat; sonst nur Top-Level-Items |
| `checks` | Prüfregeln je Typ (Untertypen erben); `follow` nennt die Eigenschaften, deren Objektwerte mit den Regeln desselben Features geprüft werden |
| `values` | erwartetes Format je Eigenschaft: `text`, `url`, `image`, `date`, `number`, `integer`, `currency`, `duration`, `enum:<Liste aus enums>` |

Befundcodes: `sd.google.missing_required` (Pflicht fehlt; bei `one_of` ein Befund für die ganze Gruppe, Eigenschaft
`a|b|c`), `sd.google.invalid_value` (Pflicht- oder empfohlene Eigenschaft vorhanden, Format ungültig),
`sd.google.missing_recommended` (Warnung). Ein erkanntes Feature wird als Seitenfakt `sd.google.feature` gespeichert.

Die Version (`version`) wird mit jeder Seite gespeichert (`sdval.rules_version`). Änderungen nur mit Release und neuer
Version.
