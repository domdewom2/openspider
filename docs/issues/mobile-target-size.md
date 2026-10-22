# Mobile: Target Size

Typ: Issue (T-W2) · Priorität: Medium (P-M1)

Abweichung von der Herleitung: benchmark-parity: TECH-233 / UI-064 ordnen die ausführungsabhängige Bedienbarkeitsprüfung als Issue ein.

## Deutsch

**Was bedeutet das?** Lighthouse beanstandet zu kleine oder zu dicht benachbarte interaktive Ziele. Der Score für Tap Targets liegt unter 100.

**Warum ist es relevant?** Kleine Ziele lassen sich auf Touchscreens schwer treffen. Nutzer können versehentlich das benachbarte Bedienelement auslösen.

**Wie beheben?** Vergrößern Sie interaktive Ziele auf mindestens 24 mal 24 CSS-Pixel oder schaffen Sie ausreichenden Abstand zu benachbarten Zielen.

## English

**What does it mean?** Lighthouse reports interactive targets that are too small or too close together. The Tap Targets score is below 100.

**Why does it matter?** Small targets are hard to hit on touchscreens and users may activate an adjacent control by mistake.

**How to fix** Make interactive targets at least 24 by 24 CSS pixels or leave sufficient space between neighboring targets.
