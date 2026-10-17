-- M2-F05a: additive Schnittstellen der Welle 1; READ_API_VERSION bleibt 1.
ALTER TABLE links ADD COLUMN crawlable INTEGER NOT NULL DEFAULT 1;
ALTER TABLE links ADD COLUMN uncrawlable_reason TEXT;
ALTER TABLE links ADD COLUMN pdf_page INTEGER;
CREATE TABLE cookies (
 url_id INTEGER NOT NULL REFERENCES urls(id) ON DELETE CASCADE, position INTEGER NOT NULL, origin TEXT NOT NULL,
 name TEXT NOT NULL, value TEXT NOT NULL, domain TEXT, path TEXT, expires TEXT, max_age INTEGER,
 secure INTEGER NOT NULL, http_only INTEGER NOT NULL, same_site TEXT, raw TEXT NOT NULL,
 PRIMARY KEY (url_id, position)
) WITHOUT ROWID;
ALTER TABLE page_html ADD COLUMN compression TEXT;
ALTER TABLE render_pages ADD COLUMN compression TEXT;
ALTER TABLE render_pages ADD COLUMN console_dropped INTEGER NOT NULL DEFAULT 0;
ALTER TABLE render_console ADD COLUMN col INTEGER;
ALTER TABLE render_console ADD COLUMN timestamp_ms INTEGER;
CREATE TABLE discovery_edges (
 seq INTEGER PRIMARY KEY, target_id INTEGER NOT NULL REFERENCES urls(id) ON DELETE CASCADE,
 source_id INTEGER REFERENCES urls(id) ON DELETE SET NULL, kind TEXT NOT NULL, link_type TEXT, anchor TEXT, depth INTEGER
);
CREATE INDEX discovery_edges_target ON discovery_edges(target_id, seq);
CREATE TABLE sitemap_entry_ext (
 sitemap_id INTEGER NOT NULL REFERENCES sitemaps(id) ON DELETE CASCADE,
 url_id INTEGER NOT NULL REFERENCES urls(id) ON DELETE CASCADE,
 position INTEGER NOT NULL, kind TEXT NOT NULL, json TEXT NOT NULL,
 PRIMARY KEY(sitemap_id, url_id, position)
) WITHOUT ROWID;
ALTER TABLE image_instances ADD COLUMN resolved TEXT;
ALTER TABLE image_instances ADD COLUMN link_position TEXT;
ALTER TABLE image_instances ADD COLUMN origin TEXT NOT NULL DEFAULT 'HTML';
ALTER TABLE image_instances ADD COLUMN real_w INTEGER;
ALTER TABLE image_instances ADD COLUMN real_h INTEGER;
ALTER TABLE image_instances ADD COLUMN display_w INTEGER;
ALTER TABLE image_instances ADD COLUMN display_h INTEGER;
ALTER TABLE image_instances ADD COLUMN in_viewport INTEGER;
ALTER TABLE image_instances ADD COLUMN bytes INTEGER;
DROP VIEW q_urls;
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
  CASE WHEN (u.content_class = 'HTML' OR (u.is_internal = 1 AND u.content_class = 'PDF')) AND u.status_code BETWEEN 200 AND 299 THEN u.word_count END AS word_count,
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
