# Hreflang: Incorrect Language & Region Codes

Typ: Issue (T-I2) · Priorität: High (P-H2)

## Deutsch

**Was bedeutet das?** Eine hreflang-Angabe verwendet einen Sprach- oder Regionscode, der nicht gültig ist.

**Warum ist es relevant?** Suchmaschinen ignorieren Angaben mit Codes, die sie nicht lesen können, daher wird die Seite nicht der gewünschten Sprache oder dem gewünschten Land zugeordnet.

**Wie beheben?** Gültige Sprachcodes (ISO 639-1) und bei Bedarf Regionscodes (ISO 3166-1 Alpha-2) wie en-GB oder de-AT verwenden. Die Region darf nicht allein stehen, der Wert x-default ist erlaubt.

## English

**What does it mean?** An hreflang annotation uses a language or region code that is not valid.

**Why does it matter?** Search engines ignore annotations with codes they cannot read, so the page is not assigned to the intended language or country.

**How to fix** Use valid language codes (ISO 639-1) and, where needed, region codes (ISO 3166-1 alpha-2) such as en-GB or de-AT. Note that the region cannot stand alone and that the value x-default is allowed.
