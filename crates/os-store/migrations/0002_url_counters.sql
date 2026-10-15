-- OpenSpider Crawl-Datei, Schema v2 (docs/plan/M1.md 3.14, K-75): Link-Zähler in einer schmalen Tabelle.
--
-- Die Zählerspalten in `urls` (inlinks … img_inlinks) bleiben im Schema, werden ab v2 aber weder gepflegt noch
-- gelesen. `url_counters` ist die einzige Quelle: Je Batch ändert der Writer die Zähler vieler zufälliger URLs; in der
-- breiten `urls`-Zeile (rund 300 Byte Text) passt die Tabelle bei 1 Mio. URLs nicht in den Cache, die schmale
-- Zählerzeile (rund 30 Byte) bleibt darin.

CREATE TABLE url_counters (
  url_id INTEGER PRIMARY KEY,
  inlinks INTEGER NOT NULL DEFAULT 0,               -- Hyperlink-Instanzen inkl. Selbstlinks (ONPAGE-015, A-9)
  unique_inlinks INTEGER NOT NULL DEFAULT 0,        -- verschiedene Quellseiten (Hyperlinks)
  self_inlinks INTEGER NOT NULL DEFAULT 0,          -- Hyperlink-Instanzen von sich selbst
  inlinks_all INTEGER NOT NULL DEFAULT 0,           -- alle Linktypen inkl. Ressourcen
  img_inlinks INTEGER NOT NULL DEFAULT 0,           -- verschiedene Seiten mit Bild-Instanz (ONPAGE-171)
  outlinks INTEGER NOT NULL DEFAULT 0,
  unique_outlinks INTEGER NOT NULL DEFAULT 0,
  self_outlinks INTEGER NOT NULL DEFAULT 0,
  external_outlinks INTEGER NOT NULL DEFAULT 0,
  unique_external_outlinks INTEGER NOT NULL DEFAULT 0,
  html_2xx_sources INTEGER NOT NULL DEFAULT 0,      -- verschiedene interne HTML-Quellseiten mit Status 2xx und >= 1 Hyperlink auf die URL (`% of Total`, ONPAGE-015/K-56)
  image_link_inlinks INTEGER NOT NULL DEFAULT 0,    -- Link-Instanzen vom Typ Image mit interner Quelle (Images-Grundmenge, ONPAGE-170)
  change_seq INTEGER NOT NULL DEFAULT 0             -- Batch-Sequenz der letzten Zähleränderung (Änderungsfeed, `queries::changed_since`)
) WITHOUT ROWID;
CREATE INDEX url_counters_change_seq ON url_counters(change_seq);

-- Jede URL hat genau eine Zählerzeile, in derselben Transaktion wie ihre `urls`-Zeile (auch bei direkten Einfügungen
-- durch Werkzeuge und Tests); sie verschwindet mit der URL.
CREATE TRIGGER urls_counters_insert AFTER INSERT ON urls
BEGIN
  INSERT OR IGNORE INTO url_counters (url_id) VALUES (new.id);
END;
CREATE TRIGGER urls_counters_delete AFTER DELETE ON urls
BEGIN
  DELETE FROM url_counters WHERE url_id = old.id;
END;

-- Übernahme der Werte bestehender Dateien (Schema v1).
INSERT OR IGNORE INTO url_counters
  (url_id, inlinks, unique_inlinks, self_inlinks, inlinks_all, img_inlinks, outlinks, unique_outlinks,
   self_outlinks, external_outlinks, unique_external_outlinks)
SELECT id, inlinks, unique_inlinks, self_inlinks, inlinks_all, img_inlinks, outlinks, unique_outlinks,
       self_outlinks, external_outlinks, unique_external_outlinks
FROM urls;

-- Die zwei neuen Zähler einmalig aus `links` berechnet (nur Links mit interner Quelle).
UPDATE url_counters SET html_2xx_sources = c.n
FROM (SELECT l.target_id AS id, COUNT(DISTINCT l.source_id) AS n
        FROM links l JOIN urls s ON s.id = l.source_id
       WHERE l.target_id IS NOT NULL AND l.link_type = 'Hyperlink' AND s.is_internal = 1
         AND s.content_class = 'HTML' AND s.status_code BETWEEN 200 AND 299
       GROUP BY l.target_id) AS c
WHERE url_counters.url_id = c.id;

UPDATE url_counters SET image_link_inlinks = c.n
FROM (SELECT l.target_id AS id, COUNT(*) AS n
        FROM links l JOIN urls s ON s.id = l.source_id
       WHERE l.target_id IS NOT NULL AND l.link_type = 'Image' AND s.is_internal = 1
       GROUP BY l.target_id) AS c
WHERE url_counters.url_id = c.id;
