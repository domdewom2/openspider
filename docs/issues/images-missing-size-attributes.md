# Images: Missing Size Attributes

Typ: Issue (T-I2) · Priorität: Low (P-L1)

Befüllt erst nach der Crawl-Analyse.

## Deutsch

**Was bedeutet das?** Bei mindestens einer Bildeinbindung fehlt eine Breiten- oder Höhenangabe im HTML.

**Warum ist es relevant?** Ohne feste Abmessungen kann sich das Seitenlayout während des Ladens verschieben.

**Wie beheben?** An jedem img-Element width und height mit dem passenden Seitenverhältnis angeben. CSS-Größen ersetzen diese Attribute nicht.

## English

**What does it mean?** At least one image instance has no width or height attribute in the HTML.

**Why does it matter?** Without dimensions, the page layout can shift while the image loads.

**How to fix** Set width and height on each img element using the correct aspect ratio. CSS dimensions do not replace these attributes.
