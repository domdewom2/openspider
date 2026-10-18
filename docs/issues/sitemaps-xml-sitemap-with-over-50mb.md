# Sitemaps: XML Sitemap With Over 50MB

Typ: Issue (T-I2) · Priorität: High (P-H1)

## Deutsch

**Was bedeutet das?** Die unkomprimierte XML-Sitemap ist größer als 52.428.800 Bytes (50 MiB).

**Warum ist es relevant?** Die Größengrenze gilt auch für gzip-Dateien nach dem Entpacken. Größere Dateien können verworfen werden.

**Wie beheben?** Teile die Sitemap in kleinere Dateien auf und prüfe jeweils die unkomprimierte Größe.

## English

**What does it mean?** The uncompressed XML sitemap exceeds 52,428,800 bytes (50 MiB).

**Why does it matter?** The size limit applies to gzip files after decompression too. Larger files may be rejected.

**How to fix** Split the sitemap into smaller files and check the uncompressed size of each file.
