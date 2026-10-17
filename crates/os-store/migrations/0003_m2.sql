-- OpenSpider Crawl-Datei, Schema v3 (docs/plan/M2.md 3.4 und 3.4a, M2-F02).
-- Regeln wie 0001: Zeitstempel als TEXT (Timestamp::to_db), Bool als INTEGER 0/1, Enum-Spalten speichern as_str().
-- Bestehende Migrationen werden nie geändert. user_version setzt der Migrationsrahmen (schema.rs).
--
-- Aufbau: (1) neue Tabellen, (2) Erweiterung bestehender Tabellen, (3) die öffentliche Leseschnittstelle
-- (OUT-233/OUT-234, `q_`-Sichten). Die Sichten liegen in dieser Migration (nicht in einer Installationsroutine beim
-- Öffnen), weil sie so mit dem Schema versioniert sind: eine neu angelegte und eine aus v1/v2 migrierte Datei
-- enthalten byte-gleiche Sichtdefinitionen, ein Leser sieht beim Öffnen keine DDL-Änderung (kein Schreibzugriff auf
-- die Datei, keine Sperre gegen einen laufenden Crawl), und die Definition ist wie `schema.rs` nach Welle 0
-- eingefroren: additive Änderungen der Version 1 kommen als neue Migration (N-Paket).

-- ---------------------------------------------------------------------------------------------------------------
-- (1) Neue Tabellen

-- Skalare Zusatzfakten je URL (os_model::ext, Schlüssel `<namensraum>.<name>`). Genau eine der drei Wertspalten ist
-- gesetzt (Bool als v_int 0/1); sind alle NULL, ist der Wert `Null`.
CREATE TABLE url_ext (
  url_id INTEGER NOT NULL, key TEXT NOT NULL,
  v_int INTEGER, v_real REAL, v_text TEXT,
  change_seq INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (url_id, key)
) WITHOUT ROWID;
CREATE INDEX url_ext_key ON url_ext(key, url_id);
CREATE INDEX url_ext_change_seq ON url_ext(change_seq);

-- Reiner Cache der View-Sitzungen (K-1a): darf jederzeit verworfen werden; `data` ist für den Store opak.
CREATE TABLE view_cache (
  key TEXT NOT NULL, params_hash TEXT NOT NULL, step_stamp TEXT NOT NULL DEFAULT '',
  built_seq INTEGER NOT NULL DEFAULT 0, data BLOB NOT NULL,
  PRIMARY KEY (key, params_hash)
) WITHOUT ROWID;

-- Bearbeitete SERP-Snippets (K-8, ONPAGE-194).
CREATE TABLE serp_edits (
  url_id INTEGER PRIMARY KEY, device TEXT NOT NULL DEFAULT 'desktop',
  title TEXT, description TEXT, edited_at TEXT NOT NULL
);

-- Rendering (CRAWL-078 … 084).
CREATE TABLE render_pages (
  url_id INTEGER PRIMARY KEY, status TEXT NOT NULL, html BLOB, word_count INTEGER,
  load_ms INTEGER NOT NULL DEFAULT 0, wait_ms INTEGER NOT NULL DEFAULT 0, total_ms INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE render_console (
  url_id INTEGER NOT NULL, position INTEGER NOT NULL, level TEXT NOT NULL, text TEXT NOT NULL,
  source TEXT, line INTEGER, PRIMARY KEY (url_id, position)
) WITHOUT ROWID;
CREATE TABLE render_blocked (
  url_id INTEGER NOT NULL, position INTEGER NOT NULL, url TEXT NOT NULL, reason TEXT NOT NULL, status INTEGER,
  PRIMARY KEY (url_id, position)
) WITHOUT ROWID;
CREATE TABLE render_network (
  url_id INTEGER NOT NULL, position INTEGER NOT NULL, url TEXT NOT NULL, status INTEGER NOT NULL,
  resource_type TEXT NOT NULL, bytes INTEGER NOT NULL DEFAULT 0, duration_ms INTEGER,
  PRIMARY KEY (url_id, position)
) WITHOUT ROWID;
CREATE TABLE screenshots (
  url_id INTEGER NOT NULL, tile INTEGER NOT NULL, png BLOB NOT NULL,
  width INTEGER NOT NULL DEFAULT 0, height INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (url_id, tile)
) WITHOUT ROWID;

-- Strukturierte Daten (TECH-180 ff.).
CREATE TABLE sd_items (
  id INTEGER PRIMARY KEY, url_id INTEGER NOT NULL, format TEXT NOT NULL, type_path TEXT NOT NULL, json TEXT NOT NULL
);
CREATE INDEX sd_items_url ON sd_items(url_id);
CREATE TABLE sd_issues (
  id INTEGER PRIMARY KEY, url_id INTEGER NOT NULL, item_id INTEGER, validation TEXT NOT NULL, feature TEXT,
  severity TEXT NOT NULL, property TEXT, code TEXT NOT NULL
);
CREATE INDEX sd_issues_url ON sd_issues(url_id);

-- Barrierefreiheit (TECH-250 ff.).
CREATE TABLE a11y_rules (
  url_id INTEGER NOT NULL, rule_id TEXT NOT NULL, status TEXT NOT NULL, impact TEXT,
  PRIMARY KEY (url_id, rule_id)
) WITHOUT ROWID;
CREATE TABLE a11y_nodes (
  id INTEGER PRIMARY KEY, url_id INTEGER NOT NULL, rule_id TEXT NOT NULL, target TEXT NOT NULL,
  snippet TEXT NOT NULL, summary TEXT NOT NULL
);
CREATE INDEX a11y_nodes_url ON a11y_nodes(url_id, rule_id);

-- Rechtschreibung und Grammatik (ONPAGE-153 ff.).
CREATE TABLE spelling_findings (
  id INTEGER PRIMARY KEY, url_id INTEGER NOT NULL, kind TEXT NOT NULL, language TEXT NOT NULL, section TEXT NOT NULL,
  text TEXT NOT NULL, detail TEXT, suggestion TEXT, rule_id TEXT
);
CREATE INDEX spelling_findings_url ON spelling_findings(url_id);

-- Benutzerdefinierte Suche, Extraktion und JavaScript (04 B – D).
CREATE TABLE custom_search_hits (
  url_id INTEGER NOT NULL, def_index INTEGER NOT NULL, count INTEGER NOT NULL DEFAULT 0, error TEXT,
  PRIMARY KEY (url_id, def_index)
) WITHOUT ROWID;
CREATE TABLE custom_extraction_values (
  url_id INTEGER NOT NULL, def_index INTEGER NOT NULL, number INTEGER NOT NULL, value TEXT NOT NULL, error TEXT,
  PRIMARY KEY (url_id, def_index, number)
) WITHOUT ROWID;
CREATE TABLE custom_js_values (
  url_id INTEGER NOT NULL, def_index INTEGER NOT NULL, number INTEGER NOT NULL, value TEXT, error TEXT,
  PRIMARY KEY (url_id, def_index, number)
) WITHOUT ROWID;

-- Formulare (ONPAGE-034/035).
CREATE TABLE forms (
  url_id INTEGER NOT NULL, idx INTEGER NOT NULL, action TEXT, method TEXT NOT NULL,
  PRIMARY KEY (url_id, idx)
) WITHOUT ROWID;

-- Nahezu-Duplikate (ONPAGE-144/145): MinHash-Signatur (Little-Endian-u32-Folge) und Treffer der Analyse.
CREATE TABLE minhash (url_id INTEGER PRIMARY KEY, sig BLOB NOT NULL);
CREATE TABLE near_duplicates (
  url_id INTEGER NOT NULL, other_id INTEGER NOT NULL, similarity REAL NOT NULL,
  PRIMARY KEY (url_id, other_id)
) WITHOUT ROWID;

-- Segmentzuordnung (K-14).
CREATE TABLE url_segments (
  url_id INTEGER NOT NULL, segment_id INTEGER NOT NULL, PRIMARY KEY (url_id, segment_id)
) WITHOUT ROWID;
CREATE INDEX url_segments_segment ON url_segments(segment_id, url_id);

-- API-Quellen (04 A, E – J).
CREATE TABLE api_values (
  url_id INTEGER NOT NULL, source TEXT NOT NULL, metric TEXT NOT NULL, level TEXT NOT NULL DEFAULT 'url',
  v_real REAL, v_text TEXT, fetched_at TEXT NOT NULL,
  PRIMARY KEY (url_id, source, metric, level)
) WITHOUT ROWID;
CREATE INDEX api_values_source ON api_values(source, metric);
CREATE TABLE api_status (
  source TEXT PRIMARY KEY, state TEXT NOT NULL, started_at TEXT, finished_at TEXT,
  urls_total INTEGER NOT NULL DEFAULT 0, urls_done INTEGER NOT NULL DEFAULT 0, urls_failed INTEGER NOT NULL DEFAULT 0,
  detail TEXT
) WITHOUT ROWID;
CREATE TABLE api_errors (
  id INTEGER PRIMARY KEY, source TEXT NOT NULL, url TEXT, code TEXT NOT NULL, message TEXT NOT NULL, at TEXT NOT NULL
);
CREATE TABLE api_urls (
  url TEXT NOT NULL, source TEXT NOT NULL, first_seen TEXT NOT NULL, PRIMARY KEY (url, source)
) WITHOUT ROWID;

-- ---------------------------------------------------------------------------------------------------------------
-- (2) Erweiterung bestehender Tabellen
--
-- `links.target`/`links.origin` aus M2.md 3.4 entfallen: `links.target_attr` (ONPAGE-036) und `links.link_origin`
-- (CRAWL-079) gibt es seit Schema v1 (0001_init.sql) und der Writer befüllt sie seit M1.

-- Fehlervermerk bei ungültigem XML (CRAWL-060, K-27).
ALTER TABLE sitemaps ADD COLUMN error TEXT;
-- CRAWL-060-Rest: changefreq, priority und die Erweiterungen (Anzahl je Eintrag).
ALTER TABLE sitemap_entries ADD COLUMN changefreq TEXT;
ALTER TABLE sitemap_entries ADD COLUMN priority REAL;
ALTER TABLE sitemap_entries ADD COLUMN images INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sitemap_entries ADD COLUMN videos INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sitemap_entries ADD COLUMN news INTEGER NOT NULL DEFAULT 0;

-- ---------------------------------------------------------------------------------------------------------------
-- (3) Öffentliche Leseschnittstelle, Version 1 (OUT-233, OUT-234)
--
-- Nur-lesende Sichten mit Präfix `q_`. Sie kommen ohne eigene Kollationen und Funktionen aus (nur eingebaute
-- SQLite-Funktionen), lesen die Zähler aus `url_counters` (nicht aus den stillgelegten Zählerspalten von `urls`) und
-- stimmen in Werten und Einheiten mit den CSV-Exporten derselben Datei überein: `q_urls` mit `Internal:All`/
-- `External:All`, `q_links` mit `All Inlinks`. Zeitpunkte: ISO 8601 mit Offset (`+00:00`, UTC), ohne Sekundenbruchteile.
-- Spaltennamen: snake_case der kanonischen Spaltennamen, Einheiten als Suffix. Änderungen nur additiv (neue Sichten,
-- neue Spalten am Ende); alles andere erhöht `read_api_version`.

-- Manifest (OUT-233): eine Zeile je Schlüssel, jeder Schlüssel ist immer vorhanden (unbekannt = NULL).
-- Hilfswerte stehen in `crawl_meta`: `config_sha256`, `user_agent`, `analysis_auto`, `openspider_version`,
-- `last_writer_version` (von os-store beim Anlegen, bei Konfigurationsänderungen und beim Öffnen zum Schreiben
-- gepflegt, meta.rs); SQLite hat keine SHA-256-Funktion.
CREATE VIEW q_manifest AS
WITH m(key, value) AS (SELECT key, NULLIF(value, '') FROM crawl_meta),
     hb(ref) AS (
       SELECT COALESCE((SELECT value FROM m WHERE key = 'heartbeat_at'),
                       (SELECT value FROM m WHERE key = 'started_at'),
                       (SELECT value FROM m WHERE key = 'created_at'))),
     st(state, alive) AS (
       SELECT COALESCE((SELECT value FROM m WHERE key = 'state'), 'Incomplete'),
              COALESCE((julianday('now') - julianday((SELECT ref FROM hb))) * 86400000.0 <= 10000.0, 0))
SELECT 'format' AS key, 'ospider' AS value
UNION ALL SELECT 'manifest_version', '1'
UNION ALL SELECT 'read_api_version', '1'
UNION ALL SELECT 'schema_version', (SELECT value FROM m WHERE key = 'schema_version')
UNION ALL SELECT 'openspider_version', (SELECT value FROM m WHERE key = 'openspider_version')
UNION ALL SELECT 'last_writer_version', (SELECT value FROM m WHERE key = 'last_writer_version')
UNION ALL SELECT 'writer_kind', (SELECT value FROM m WHERE key = 'owner_kind')
UNION ALL SELECT 'crawl_id', (SELECT value FROM m WHERE key = 'crawl_id')
UNION ALL SELECT 'name', (SELECT value FROM m WHERE key = 'name')
UNION ALL SELECT 'project', (SELECT value FROM m WHERE key = 'project')
UNION ALL SELECT 'mode', (SELECT value FROM m WHERE key = 'mode')
UNION ALL SELECT 'start_url', (SELECT value FROM m WHERE key = 'start_url')
UNION ALL SELECT 'list_source', (SELECT value FROM m WHERE key = 'list_source')
UNION ALL SELECT 'started_at', strftime('%Y-%m-%dT%H:%M:%S+00:00', (SELECT value FROM m WHERE key = 'started_at'))
UNION ALL SELECT 'ended_at',
  CASE WHEN (SELECT state FROM st) IN ('Running', 'Paused') THEN NULL
       ELSE strftime('%Y-%m-%dT%H:%M:%S+00:00', (SELECT value FROM m WHERE key = 'ended_at')) END
UNION ALL SELECT 'state',
  CASE WHEN (SELECT state FROM st) = 'Running' AND NOT (SELECT alive FROM st) THEN 'Incomplete'
       ELSE (SELECT state FROM st) END
UNION ALL SELECT 'end_reason',
  CASE WHEN (SELECT state FROM st) IN ('Running', 'Paused') THEN NULL
       ELSE (SELECT value FROM m WHERE key = 'end_reason') END
UNION ALL SELECT 'completeness',
  CASE (SELECT state FROM st)
    WHEN 'Completed' THEN CASE (SELECT value FROM m WHERE key = 'end_reason')
                            WHEN 'queue_empty' THEN 'complete'
                            WHEN 'limit_reached' THEN 'limited'
                            WHEN 'stopped' THEN 'stopped'
                            ELSE 'interrupted' END
    WHEN 'Paused' THEN 'paused'
    WHEN 'Running' THEN CASE WHEN (SELECT alive FROM st) THEN 'running' ELSE 'interrupted' END
    WHEN 'Failed' THEN 'failed'
    ELSE 'interrupted' END
UNION ALL SELECT 'user_agent', (SELECT value FROM m WHERE key = 'user_agent')
UNION ALL SELECT 'config_sha256', (SELECT value FROM m WHERE key = 'config_sha256')
UNION ALL SELECT 'urls_total', (SELECT CAST(COUNT(*) AS TEXT) FROM urls WHERE in_tables = 1)
UNION ALL SELECT 'urls_crawled', (SELECT CAST(COUNT(*) AS TEXT) FROM urls WHERE in_tables = 1 AND state = 'done')
UNION ALL SELECT 'analysis_state',
  CASE WHEN (SELECT value FROM m WHERE key = 'analysis_state') = 'completed' THEN 'done'
       WHEN (SELECT value FROM m WHERE key = 'analysis_state') = 'stale' THEN 'stale'
       WHEN (SELECT value FROM m WHERE key = 'analysis_state') IS NULL
            AND (SELECT value FROM m WHERE key = 'analysis_auto') = '0' THEN 'off'
       ELSE 'pending' END;

CREATE VIEW q_config AS
SELECT osconfig AS osconfig, strftime('%Y-%m-%dT%H:%M:%S+00:00', updated_at) AS updated_at FROM crawl_config;

-- Eine Zeile je URL, die in den Tabellen erscheint (intern und extern). Die Ausdrücke der Element-Spalten entsprechen
-- dem Tab „Internal“ (nur interne HTML-Seiten bzw. PDFs mit Status 2xx tragen Titel, Überschriften, Wortzahl).
CREATE VIEW q_urls AS
SELECT
  u.id AS url_id,
  u.address AS address,
  u.is_internal AS is_internal,
  CASE WHEN u.state = 'done' THEN 1 ELSE 0 END AS crawled,
  u.content_type AS content_type,
  u.status_code AS status_code,
  u.status AS status,
  u.indexability AS indexability,
  u.indexability_status AS indexability_status,
  CASE WHEN u.content_class IN ('HTML', 'PDF') AND u.status_code BETWEEN 200 AND 299 THEN u.title_1 END AS title_1,
  CASE WHEN u.content_class = 'HTML' AND u.status_code BETWEEN 200 AND 299 THEN u.meta_desc_1 END AS meta_description_1,
  CASE WHEN u.content_class = 'HTML' AND u.status_code BETWEEN 200 AND 299 THEN u.h1_1 END AS h1_1,
  CASE WHEN u.content_class = 'HTML' AND u.status_code BETWEEN 200 AND 299 THEN u.h2_1 END AS h2_1,
  (SELECT COALESCE(k.resolved, k.raw) FROM canonicals k
    WHERE k.url_id = u.id AND k.source = 'html' ORDER BY k.position LIMIT 1) AS canonical_link_element_1,
  CASE WHEN u.content_class = 'HTML' AND u.status_code BETWEEN 200 AND 299 THEN u.word_count END AS word_count,
  CASE WHEN u.size_bytes IS NULL OR u.content_class IN ('HTML', 'Images') THEN u.size_bytes
       ELSE COALESCE((SELECT CAST(h.value AS INTEGER) FROM http_headers h
                       WHERE h.url_id = u.id AND h.direction = 'response' AND lower(h.name) = 'content-length'
                       LIMIT 1), 0) END AS size_bytes,
  u.transferred_bytes AS transferred_bytes,
  u.response_time_ms / 1000.0 AS response_time,
  u.crawl_depth AS crawl_depth,
  u.folder_depth AS folder_depth,
  c.inlinks AS inlinks,
  c.unique_inlinks AS unique_inlinks,
  c.outlinks AS outlinks,
  c.unique_outlinks AS unique_outlinks,
  c.external_outlinks AS external_outlinks,
  u.redirect_url AS redirect_url,
  CASE u.redirect_type WHEN 'http' THEN 'HTTP Redirect' WHEN 'hsts_policy' THEN 'HSTS Policy'
       WHEN 'javascript' THEN 'JavaScript Redirect' WHEN 'meta_refresh' THEN 'Meta Refresh Redirect'
       WHEN 'http_refresh' THEN 'HTTP Refresh Redirect' ELSE NULL END AS redirect_type,
  CASE WHEN u.http_version LIKE 'HTTP/%' THEN substr(u.http_version, 6) ELSE u.http_version END AS http_version,
  u.hash_md5 AS hash,
  strftime('%Y-%m-%dT%H:%M:%S+00:00', u.last_modified) AS last_modified,
  strftime('%Y-%m-%dT%H:%M:%S+00:00', u.crawl_timestamp) AS crawl_timestamp
FROM urls u LEFT JOIN url_counters c ON c.url_id = u.id
WHERE u.in_tables = 1;

-- Eine Zeile je Link-Instanz (Hyperlinks und Ressourcen-Referenzen), wie der Export „All Inlinks“.
CREATE VIEW q_links AS
SELECT
  l.source_id AS source_url_id,
  l.target_id AS target_url_id,
  s.address AS source_address,
  COALESCE(t.address, l.target_raw) AS target_address,
  l.link_type AS type,
  l.anchor AS anchor,
  l.alt_text AS alt_text,
  l.follow AS follow,
  l.rel AS rel,
  l.target_attr AS target,
  l.path_type AS path_type,
  l.link_path AS link_path,
  l.link_position AS link_position,
  l.link_origin AS link_origin,
  CASE WHEN t.state = 'done' AND t.status_code > 0 THEN t.status_code END AS target_status_code
FROM links l
JOIN urls s ON s.id = l.source_id
LEFT JOIN urls t ON t.id = l.target_id;

-- Eine Zeile je Eintrag einer gelesenen XML-Sitemap.
CREATE VIEW q_sitemap_urls AS
SELECT
  sm.url AS sitemap_url,
  e.url_id AS url_id,
  u.address AS address,
  e.lastmod AS lastmod
FROM sitemap_entries e
JOIN sitemaps sm ON sm.id = e.sitemap_id
JOIN urls u ON u.id = e.url_id;

-- Selbstbeschreibung aller `q_`-Sichten; ein Test hält sie lückenlos (jede Spalte jeder Sicht).
CREATE VIEW q_columns AS
SELECT column1 AS "view", column2 AS "column", column3 AS type, column4 AS description FROM (VALUES
  ('q_manifest', 'key', 'TEXT', 'Schlüssel des Manifests (OUT-233); jeder Schlüssel der Tabelle ist immer vorhanden.'),
  ('q_manifest', 'value', 'TEXT', 'Wert als Text; unbekannt oder nicht zutreffend ist NULL.'),
  ('q_config', 'osconfig', 'TEXT', 'Eingebettete Konfiguration des Crawls (TOML, .osconfig) ohne Geheimnisse.'),
  ('q_config', 'updated_at', 'TEXT', 'Zeitpunkt der letzten Änderung der Konfiguration (ISO 8601 mit Offset).'),
  ('q_urls', 'url_id', 'INTEGER', 'Schlüssel der URL, stabil innerhalb der Datei, nicht über Crawls hinweg.'),
  ('q_urls', 'address', 'TEXT', 'Adresse der URL in Anzeigeform (Spalte Address).'),
  ('q_urls', 'is_internal', 'INTEGER', '1 = interne URL, 0 = externe URL.'),
  ('q_urls', 'crawled', 'INTEGER', '1 = die URL wurde abgerufen, 0 = entdeckt oder eingeplant.'),
  ('q_urls', 'content_type', 'TEXT', 'Wert des Content-Type-Headers der Antwort.'),
  ('q_urls', 'status_code', 'INTEGER', 'HTTP-Statuscode; 0 = keine Antwort.'),
  ('q_urls', 'status', 'TEXT', 'Statustext der Antwort oder Grund einer fehlenden Antwort.'),
  ('q_urls', 'indexability', 'TEXT', 'Indexable oder Non-Indexable.'),
  ('q_urls', 'indexability_status', 'TEXT', 'Begründung der Nicht-Indexierbarkeit.'),
  ('q_urls', 'title_1', 'TEXT', 'Erster Seitentitel (HTML und PDF mit Status 2xx).'),
  ('q_urls', 'meta_description_1', 'TEXT', 'Erste Meta-Description (HTML mit Status 2xx).'),
  ('q_urls', 'h1_1', 'TEXT', 'Erste H1-Überschrift (HTML mit Status 2xx).'),
  ('q_urls', 'h2_1', 'TEXT', 'Erste H2-Überschrift (HTML mit Status 2xx).'),
  ('q_urls', 'canonical_link_element_1', 'TEXT', 'Erstes Canonical-Element im HTML, absolut aufgelöst.'),
  ('q_urls', 'word_count', 'INTEGER', 'Wortanzahl im Inhaltsbereich (HTML mit Status 2xx).'),
  ('q_urls', 'size_bytes', 'INTEGER', 'Größe der Antwort in Byte (Spalte Size (bytes)).'),
  ('q_urls', 'transferred_bytes', 'INTEGER', 'Übertragene Bytes des Bodys.'),
  ('q_urls', 'response_time', 'REAL', 'Antwortzeit in Sekunden.'),
  ('q_urls', 'crawl_depth', 'INTEGER', 'Klicktiefe ab der Start-URL.'),
  ('q_urls', 'folder_depth', 'INTEGER', 'Anzahl der Ordnerebenen im Pfad.'),
  ('q_urls', 'inlinks', 'INTEGER', 'Hyperlink-Instanzen interner Quellseiten auf die URL (Selbstlinks eingeschlossen).'),
  ('q_urls', 'unique_inlinks', 'INTEGER', 'Verschiedene interne Quellseiten mit einem Hyperlink auf die URL.'),
  ('q_urls', 'outlinks', 'INTEGER', 'Hyperlink-Instanzen der Seite auf andere URLs.'),
  ('q_urls', 'unique_outlinks', 'INTEGER', 'Verschiedene Ziele der Hyperlinks der Seite.'),
  ('q_urls', 'external_outlinks', 'INTEGER', 'Hyperlink-Instanzen der Seite auf externe URLs.'),
  ('q_urls', 'redirect_url', 'TEXT', 'Ziel der Weiterleitung.'),
  ('q_urls', 'redirect_type', 'TEXT', 'Art der Weiterleitung (HTTP Redirect, Meta Refresh Redirect, …).'),
  ('q_urls', 'http_version', 'TEXT', 'HTTP-Version der Antwort ohne Präfix (1.1, 2).'),
  ('q_urls', 'hash', 'TEXT', 'MD5 des Bodys (Hex, Kleinbuchstaben).'),
  ('q_urls', 'last_modified', 'TEXT', 'Last-Modified der Antwort (ISO 8601 mit Offset); NULL, wenn nicht lesbar.'),
  ('q_urls', 'crawl_timestamp', 'TEXT', 'Zeitpunkt des Abrufs (ISO 8601 mit Offset).'),
  ('q_links', 'source_url_id', 'INTEGER', 'url_id der Quellseite des Links.'),
  ('q_links', 'target_url_id', 'INTEGER', 'url_id des Ziels; NULL, wenn das Ziel keine URL der Datei ist.'),
  ('q_links', 'source_address', 'TEXT', 'Adresse der Quellseite.'),
  ('q_links', 'target_address', 'TEXT', 'Adresse des Ziels; ohne auflösbares Ziel der Rohwert aus dem Quelltext.'),
  ('q_links', 'type', 'TEXT', 'Linktyp (Hyperlink, Image, JavaScript, CSS, …).'),
  ('q_links', 'anchor', 'TEXT', 'Ankertext.'),
  ('q_links', 'alt_text', 'TEXT', 'Alt-Text eines Bildes im Link.'),
  ('q_links', 'follow', 'INTEGER', '1 = der Link ist follow, 0 = nofollow.'),
  ('q_links', 'rel', 'TEXT', 'Wert des rel-Attributs.'),
  ('q_links', 'target', 'TEXT', 'Wert des target-Attributs.'),
  ('q_links', 'path_type', 'TEXT', 'Pfadart des Links (Absolute, Root-Relative, …).'),
  ('q_links', 'link_path', 'TEXT', 'XPath des Link-Elements im Dokument.'),
  ('q_links', 'link_position', 'TEXT', 'Position im Dokument (Navigation, Content, Footer, …).'),
  ('q_links', 'link_origin', 'TEXT', 'Herkunft des Links (HTML oder gerendertes HTML).'),
  ('q_links', 'target_status_code', 'INTEGER', 'Statuscode des Ziels; NULL ohne Abruf oder Antwort.'),
  ('q_sitemap_urls', 'sitemap_url', 'TEXT', 'Adresse der Sitemap-Datei, in der der Eintrag steht.'),
  ('q_sitemap_urls', 'url_id', 'INTEGER', 'url_id der URL des Eintrags.'),
  ('q_sitemap_urls', 'address', 'TEXT', 'Adresse der URL des Eintrags.'),
  ('q_sitemap_urls', 'lastmod', 'TEXT', 'Wert von lastmod aus der Sitemap (roh).'),
  ('q_columns', 'view', 'TEXT', 'Name der Sicht.'),
  ('q_columns', 'column', 'TEXT', 'Name der Spalte.'),
  ('q_columns', 'type', 'TEXT', 'Typ der Spalte: INTEGER, REAL oder TEXT.'),
  ('q_columns', 'description', 'TEXT', 'Bedeutung der Spalte in einem Satz.')
);

