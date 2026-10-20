# schema.org-Vokabular (Datenasset)

`vocabulary.json` ist eine Bearbeitung des schema.org-Vokabulars, Version 30.1 vom 16.09.2026
(Quelle: <https://schema.org/version/30.1/schemaorg-current-https.jsonld>, Kern und `pending`).
Übernommen sind nur Typnamen, Oberklassen, Datentypen sowie Domain (`domainIncludes`) und Range (`rangeIncludes`)
der Eigenschaften; Beschreibungen und Kommentare sind entfernt. Erzeugt mit `build_vocab.py` in diesem Ordner.

- Urheber: schema.org (Sponsoren und Mitwirkende, <https://schema.org/docs/about.html>)
- Lizenz: Creative Commons Attribution-ShareAlike 3.0 Unported (CC BY-SA 3.0),
  <https://creativecommons.org/licenses/by-sa/3.0/>
- Weitergabe dieses Assets nur unter derselben Lizenz. Die Lizenz gilt nur für diese Datendatei, nicht für den
  Programmcode von OpenSpider (Apache-2.0). Nicht von `cargo deny` erfasst: manuelle Lizenzprüfung im Release-Prozess
  (`docs/spec/03-tabs-technical.md`, Anhang C).

Aktualisierung nur mit einem Release: neue Quelldatei laden, `build_vocab.py` ausführen, Version im Dateinamen der
Quelle und in dieser Datei nachziehen.
