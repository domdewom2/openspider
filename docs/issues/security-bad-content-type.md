# Security: Bad Content Type

Typ: Warning (T-W2) · Priorität: Low (P-L1)

## Deutsch

**Was bedeutet das?** Der Content-Type ist ungültig, als veraltet konfiguriert oder widerspricht Dateiendung beziehungsweise Inhaltssignatur.

**Warum ist es relevant?** Ein falscher Medientyp kann Darstellung und Verarbeitung der Ressource verändern.

**Wie beheben?** Den tatsächlichen Inhalt mit dem passenden MIME-Typ ausliefern. Die Liste veralteter Typen ist konfigurierbar; text/javascript ist nach RFC 9239 ein Standardtyp.

## English

**What does it mean?** The Content-Type is invalid, configured as obsolete, or conflicts with the extension or content signature.

**Why does it matter?** An incorrect media type can change how the resource is displayed or processed.

**How to fix** Serve the actual content with the appropriate MIME type. The obsolete list is configurable; text/javascript is a standard type under RFC 9239.
