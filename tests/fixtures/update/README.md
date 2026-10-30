# Update-Fixtures (M3)

`settings.toml`: app-weite Update-Einstellung für OUT-222/OUT-250, einschließlich Zeitstempel.
M3-U01 ergänzt ausschließlich hier ein ausschließlich für Tests erzeugtes Schlüsselpaar und signierte
Manifest-/Artefakt-Fixtures; Testschlüssel dürfen nie in Produkt-Konfigurationen gelangen.
Der Mock in os-testsite liefert bis dahin strukturelle Manifest-Gerüste, keine gültigen Signaturen.
