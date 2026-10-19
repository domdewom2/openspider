# Security: Form on HTTP URL

Typ: Issue (T-I4) · Priorität: High (P-H3)

## Deutsch

**Was bedeutet das?** Auf einer HTTP-Seite befindet sich ein Formular.

**Warum ist es relevant?** Auch bei sicherem Formularziel kann das unverschlüsselt ausgelieferte Formular manipuliert werden.

**Wie beheben?** Die gesamte Formularseite über HTTPS ausliefern und HTTP dauerhaft umleiten.

## English

**What does it mean?** This page contains a form and is delivered over unencrypted HTTP.

**Why does it matter?** Even with a secure action, a form delivered without encryption can be modified in transit.

**How to fix** Serve the entire form page over HTTPS and permanently redirect HTTP requests.
