## Stored extraction contract

`sd_items.json` contains a JSON-LD-style object for each top-level item. Types are full IRIs where a context is available; Microdata/RDFa use type arrays. Nested items remain nested. `sd_items.type_path` contains top-level short names separated by `|`. Blank nodes are allocated per URL during triple export in extraction order.

Reserved metadata keys begin with `_os_`: `_os_order` lists object property names in source order; `_os_types` contains top-level short names; `_os_source` contains JSON-LD block/document line or the HTML element path. A parse failure is an item with an empty type path and `_os_parse_error` containing `message`, `block`, `line`, `column`, `snippet`. Its corresponding `sd_issues` entry uses validation `parse` and references that item. Consumers must exclude reserved keys from schema validation and RDF statements, and traverse `_os_order` before remaining properties.

JSON-LD references are resolved across blocks within the document, retaining `@id`. Cycles stop expansion at an already visited identifier; no remote contexts are fetched. Recursion is bounded. Schema.org and Rich Result validation, finding messages and validation export annotations belong to M2-D20. App commands and visual tree interactions belong to K09/U21.

CSV/XLSX details use the selection-scoped item/finding table. The tree export writes JSON when its destination ends in `.json` or its request parameters contain `format: "json"`; the existing `tree: true` dispatcher invokes it. The JSON includes each selected item once, in format/source order.
