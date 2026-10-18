# Directives: Outside <head>

Typ: Issue (T-I2) · Priorität: High (P-H1)

## Deutsch

**Was bedeutet das?** Mindestens ein Robots-Meta-Element liegt außerhalb des HTML-Heads.

**Warum ist es relevant?** Crawler können Angaben außerhalb des Heads unterschiedlich auswerten. OpenSpider berücksichtigt sie vorsorglich.

**Wie beheben?** Verschiebe Robots-Meta-Elemente in den Head und prüfe Elemente, die den Head vorzeitig schließen.

## English

**What does it mean?** At least one robots meta element is outside the HTML head.

**Why does it matter?** Crawlers may interpret directives outside the head differently. OpenSpider applies them as a precaution.

**How to fix** Move robots meta elements into the head and check for elements that close the head prematurely.
