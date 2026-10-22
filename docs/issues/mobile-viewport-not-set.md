# Mobile: Viewport Not Set

Typ: Issue (T-I2) · Priorität: High (P-H3)

## Deutsch

**Was bedeutet das?** Der Seite fehlt eine wirksame Viewport-Angabe für die mobile Darstellung. Lighthouse meldet dafür einen Score unter 100.

**Warum ist es relevant?** Ohne passenden Viewport wird das Desktop-Layout verkleinert. Inhalte und Bedienelemente können auf kleinen Geräten unbenutzbar werden.

**Wie beheben?** Setzen Sie im Head ein Viewport-Meta mit width=device-width und prüfen Sie das Layout auf einem schmalen Bildschirm.

## English

**What does it mean?** The page has no effective viewport declaration for mobile display. Lighthouse reports a score below 100 for this audit.

**Why does it matter?** Without an appropriate viewport the desktop layout is scaled down, which can make content and controls unusable on small devices.

**How to fix** Add a viewport meta tag with width=device-width in the head and check the layout on a narrow screen.
