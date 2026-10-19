# M2-D06 Directives

Eigene, synthetische Seiten für TECH-057–059, TECH-061–063, TECH-065/008,
TECH-051/066/068, CRAWL-089/TECH-009 und OUT-055.

`filters.json` hält die erwarteten Filtermengen als portunabhängige URL-Pfade fest.
Meta- und HTTP-Angaben, Fehlerantwort und PDF-Header, abweichende Agenten,
ungültige Werte, Body-Attribute als Negativfälle sowie Refresh-Varianten sind enthalten.

Die Fachtests setzen `validation.checked` gemäß dem F05-Vertrag selbst.
Die Integration mit dem Marker-Produzenten D10 und der Sichtbarkeit aus E01
steht vollständig in `os-analyze/tests/pending_w1_d06.rs` (C9/K2-26).

`attributes.html` enthält absichtlich ein zweites `</head>` und einen zweiten
`<body>`-Start: Die HTML-Baumkorrektur darf aus `translate="no"` und
`data-nosnippet` keine Robots-Direktiven machen. Die Datei ist ein Negativfall
für Attribute, auch bei fehlerhafter HTML-Struktur.
