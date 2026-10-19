# Structured data UI fixture

Scenario: `ui/fx-ui-sd`, start `/index.html`. Enable JSON-LD, Microdata and RDFa. The index links twelve local pages: a readable Product, a Product missing its required name, a Product missing its recommended brand and a parse error for each format. The page names identify the cases for UI tests. The index itself has no structured data.

All data is written from the repository specifications. External-looking IRIs are property values; the fixture needs no external server. JSON-LD uses an invalid trailing comma, Microdata an invalid relative itemtype, and RDFa an unresolved typeof for the parse-error pages. Valid items have a nested Offer. The extraction/parse-error contracts are TECH-180–186/189; exact Schema.org/Rich Result findings are supplied and asserted by M2-D20, while the detail commands and visual interactions belong to K09/U21.
