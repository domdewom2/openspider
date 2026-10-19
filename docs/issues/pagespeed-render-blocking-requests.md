# PageSpeed: Render Blocking Requests

Typ: Opportunity (T-O2) · Priorität: Medium (P-M1)

## Deutsch

**Was bedeutet das?** Stylesheets oder Skripte halten das erste Rendern der Seite auf.

**Warum ist es relevant?** Solange blockierende Ressourcen laden, sieht der Nutzer nichts; das erste Rendern verzögert sich messbar.

**Wie beheben?** Binde kritisches CSS inline ein, lade übriges CSS nachrangig und versieh Skripte mit defer oder async.

## English

**What does it mean?** Stylesheets or scripts hold up the first render of the page.

**Why does it matter?** While blocking resources load the user sees nothing, so first render is measurably delayed.

**How to fix** Inline critical CSS, load the remaining CSS later and mark scripts with defer or async.
