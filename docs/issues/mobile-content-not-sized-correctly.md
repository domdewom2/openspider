# Mobile: Content Not Sized Correctly

Typ: Issue (T-W2) · Priorität: High (P-H3)

Abweichung von der Herleitung: benchmark-parity: TECH-234 / UI-064 stufen den ausführungsabhängigen Ausschluss mobiler Nutzer als Issue ein.

## Deutsch

**Was bedeutet das?** Der Inhalt ist laut lokalem Lighthouse-Audit breiter als der Viewport. Der Score für Content Width liegt unter 100.

**Warum ist es relevant?** Horizontales Scrollen erschwert die Bedienung auf kleinen Geräten und kann wesentliche Inhalte außerhalb des sichtbaren Bereichs verbergen.

**Wie beheben?** Entfernen Sie starre Inhaltsbreiten, begrenzen Sie Bilder und Einbettungen auf die verfügbare Breite und prüfen Sie das responsive Layout.

## English

**What does it mean?** The local Lighthouse audit reports content wider than the viewport. The Content Width score is below 100.

**Why does it matter?** Horizontal scrolling makes small screens difficult to use and can hide essential content outside the visible area.

**How to fix** Remove fixed content widths, constrain images and embeds to the available width and check the responsive layout.
