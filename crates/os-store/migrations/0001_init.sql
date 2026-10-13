-- OpenSpider Crawl-Datei, Schema v1 (docs/plan/M1.md 3.5).
-- Regeln: Zeitstempel als TEXT (Timestamp::to_db), Bool als INTEGER 0/1, Enum-Spalten speichern as_str().
-- Bestehende Migrationen werden nie geändert; Schemaänderungen kommen als neue Datei 000N_*.sql.
-- application_id und user_version setzt der Migrationsrahmen (schema.rs), nicht diese Datei.

CREATE TABLE crawl_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
-- Schlüssel: crawl_id, schema_version, generator, created_at, mode, start_url, list_source, project, name,
--            state, end_reason, started_at, ended_at, paused_at, heartbeat_at, next_url_id, next_seq,
--            analysis_state, analysis_at, analysis_stale,
--            change_seq (Nachtrag P00c: Änderungsfeed, vom Writer je Batch erhöht), reset_seq (P19: Löschungen → Caches verwerfen),
--            owner_kind (app|cli|mcp-stdio), owner_pid (P15/P19, F-14 des M2-Konzepts)
CREATE TABLE crawl_config (id INTEGER PRIMARY KEY CHECK (id = 1), osconfig TEXT NOT NULL, updated_at TEXT NOT NULL);

CREATE TABLE urls (
  id INTEGER PRIMARY KEY, url TEXT NOT NULL UNIQUE, address TEXT NOT NULL, origin TEXT NOT NULL,
  is_internal INTEGER NOT NULL, in_tables INTEGER NOT NULL DEFAULT 1,
  state TEXT NOT NULL, not_crawled_reason TEXT, discovered_via TEXT NOT NULL,
  crawl_depth INTEGER, folder_depth INTEGER NOT NULL, url_length INTEGER NOT NULL,
  crawl_timestamp TEXT, status_code INTEGER, status TEXT, no_response_reason TEXT,
  content_type TEXT, content_class TEXT NOT NULL DEFAULT 'Unknown', http_version TEXT,
  size_bytes INTEGER, transferred_bytes INTEGER, total_transferred_bytes INTEGER, response_time_ms INTEGER,
  last_modified_raw TEXT, last_modified TEXT,
  redirect_url TEXT, redirect_target_id INTEGER, redirect_type TEXT, meta_refresh_delay INTEGER,
  hash_md5 TEXT, charset TEXT,
  robots_blocked INTEGER NOT NULL DEFAULT 0, robots_matched_line TEXT, robots_txt_url TEXT,
  title_count INTEGER, title_1 TEXT, title_1_len INTEGER, title_1_px INTEGER, title_1_in_head INTEGER,
                       title_2 TEXT, title_2_len INTEGER, title_2_px INTEGER, title_2_in_head INTEGER,
  meta_desc_count INTEGER, meta_desc_1 TEXT, meta_desc_1_len INTEGER, meta_desc_1_px INTEGER, meta_desc_1_in_head INTEGER,
                           meta_desc_2 TEXT, meta_desc_2_len INTEGER, meta_desc_2_px INTEGER, meta_desc_2_in_head INTEGER,
  meta_kw_count INTEGER, meta_kw_1 TEXT, meta_kw_1_len INTEGER, meta_kw_2 TEXT, meta_kw_2_len INTEGER,
  h1_count INTEGER, h1_1 TEXT, h1_1_len INTEGER, h1_2 TEXT, h1_2_len INTEGER,
  h2_count INTEGER, h2_1 TEXT, h2_1_len INTEGER, h2_2 TEXT, h2_2_len INTEGER,
  word_count INTEGER, text_ratio REAL,
  canonical_count INTEGER NOT NULL DEFAULT 0, canonical_url TEXT, canonical_target_id INTEGER, canonical_self INTEGER,
  effective_index INTEGER, effective_follow INTEGER, has_rel_next INTEGER, has_rel_prev INTEGER,
  hreflang_count INTEGER NOT NULL DEFAULT 0,
  indexability TEXT, indexability_status TEXT,
  -- Zähler, vom Writer gepflegt (nur Links mit interner Quelle):
  inlinks INTEGER NOT NULL DEFAULT 0,          -- Hyperlink-Instanzen inkl. Selbstlinks (ONPAGE-015, A-9)
  unique_inlinks INTEGER NOT NULL DEFAULT 0,   -- verschiedene Quellseiten (Hyperlinks)
  self_inlinks INTEGER NOT NULL DEFAULT 0,     -- Hyperlink-Instanzen von sich selbst (TECH-130 zieht ab)
  inlinks_all INTEGER NOT NULL DEFAULT 0,      -- alle Linktypen inkl. Ressourcen
  outlinks INTEGER NOT NULL DEFAULT 0, unique_outlinks INTEGER NOT NULL DEFAULT 0, self_outlinks INTEGER NOT NULL DEFAULT 0,
  external_outlinks INTEGER NOT NULL DEFAULT 0, unique_external_outlinks INTEGER NOT NULL DEFAULT 0,
  img_inlinks INTEGER NOT NULL DEFAULT 0,      -- verschiedene Seiten mit Bild-Instanz (ONPAGE-171)
  change_seq INTEGER NOT NULL DEFAULT 0        -- Nachtrag P00c: Batch-Sequenz der letzten Änderung dieser Zeile (Änderungsfeed, K-2 des M2-Konzepts)
);
CREATE INDEX urls_change_seq ON urls(change_seq);  -- Nachtrag P00c
CREATE INDEX urls_status ON urls(status_code); CREATE INDEX urls_class ON urls(is_internal, content_class);
CREATE INDEX urls_state ON urls(state);        CREATE INDEX urls_depth ON urls(crawl_depth);
CREATE INDEX urls_title1 ON urls(title_1);     CREATE INDEX urls_md1 ON urls(meta_desc_1);
CREATE INDEX urls_h1 ON urls(h1_1);            CREATE INDEX urls_h2 ON urls(h2_1);
CREATE INDEX urls_hash ON urls(hash_md5);      CREATE INDEX urls_redirect ON urls(redirect_target_id);
CREATE INDEX urls_canon ON urls(canonical_target_id); CREATE INDEX urls_index ON urls(indexability);

CREATE TABLE links (
  id INTEGER PRIMARY KEY, source_id INTEGER NOT NULL, target_id INTEGER, target_raw TEXT NOT NULL,
  link_type TEXT NOT NULL, position INTEGER NOT NULL, anchor TEXT, alt_text TEXT, follow INTEGER NOT NULL,
  rel TEXT, target_attr TEXT, path_type TEXT, link_path TEXT, link_position TEXT,
  link_origin TEXT NOT NULL DEFAULT 'HTML', in_head INTEGER);
CREATE INDEX links_source ON links(source_id, position); CREATE INDEX links_target ON links(target_id, link_type);

CREATE TABLE image_instances (
  id INTEGER PRIMARY KEY, page_id INTEGER NOT NULL, image_id INTEGER, src_raw TEXT NOT NULL, source TEXT NOT NULL,
  alt_present INTEGER NOT NULL, alt TEXT, alt_len INTEGER, width_attr TEXT, height_attr TEXT, link_path TEXT, position INTEGER NOT NULL);
CREATE INDEX img_page ON image_instances(page_id); CREATE INDEX img_image ON image_instances(image_id);

CREATE TABLE canonicals (url_id INTEGER NOT NULL, source TEXT NOT NULL, position INTEGER NOT NULL, raw TEXT NOT NULL,
  resolved TEXT, target_id INTEGER, in_head INTEGER NOT NULL, PRIMARY KEY (url_id, source, position)) WITHOUT ROWID;
CREATE TABLE directives (url_id INTEGER NOT NULL, source TEXT NOT NULL, position INTEGER NOT NULL, agent TEXT, raw TEXT NOT NULL,
  tokens TEXT NOT NULL /* normalisiert, ","-getrennt */, applies INTEGER NOT NULL, in_head INTEGER NOT NULL,
  PRIMARY KEY (url_id, source, position)) WITHOUT ROWID;
CREATE TABLE refresh (url_id INTEGER NOT NULL, source TEXT NOT NULL, position INTEGER NOT NULL, raw TEXT NOT NULL, delay INTEGER,
  target_raw TEXT, target_url TEXT, target_id INTEGER, in_head INTEGER NOT NULL, PRIMARY KEY (url_id, source, position)) WITHOUT ROWID;
CREATE TABLE hreflang (url_id INTEGER NOT NULL, source TEXT NOT NULL, position INTEGER NOT NULL, sitemap_id INTEGER,
  code_raw TEXT NOT NULL, code_norm TEXT NOT NULL, code_valid INTEGER NOT NULL, invalid_reason TEXT,
  href_raw TEXT NOT NULL, url TEXT, target_id INTEGER, in_head INTEGER NOT NULL,
  PRIMARY KEY (url_id, source, position)) WITHOUT ROWID;
CREATE INDEX hreflang_target ON hreflang(target_id);
CREATE TABLE pagination (url_id INTEGER NOT NULL, rel TEXT NOT NULL, source TEXT NOT NULL, position INTEGER NOT NULL,
  raw TEXT NOT NULL, url TEXT, target_id INTEGER, in_head INTEGER NOT NULL, PRIMARY KEY (url_id, rel, source, position)) WITHOUT ROWID;
CREATE TABLE http_headers (url_id INTEGER NOT NULL, direction TEXT NOT NULL, position INTEGER NOT NULL, name TEXT NOT NULL,
  value TEXT NOT NULL, PRIMARY KEY (url_id, direction, position)) WITHOUT ROWID;

CREATE TABLE robots_files (origin TEXT PRIMARY KEY, robots_url TEXT NOT NULL, fetch_status TEXT NOT NULL, status_code INTEGER,
  attempts INTEGER NOT NULL, body TEXT, note TEXT, fetched_at TEXT) WITHOUT ROWID;
CREATE TABLE sitemaps (id INTEGER PRIMARY KEY, url TEXT NOT NULL UNIQUE, url_id INTEGER, parent_id INTEGER, kind TEXT NOT NULL,
  entries INTEGER NOT NULL DEFAULT 0, invalid_entries INTEGER NOT NULL DEFAULT 0, size_bytes INTEGER, status_code INTEGER,
  depth INTEGER NOT NULL, found_via TEXT NOT NULL);
CREATE TABLE sitemap_entries (sitemap_id INTEGER NOT NULL, url_id INTEGER NOT NULL, loc_raw TEXT NOT NULL, lastmod TEXT,
  PRIMARY KEY (sitemap_id, url_id)) WITHOUT ROWID;
CREATE INDEX sitemap_entries_url ON sitemap_entries(url_id);

CREATE TABLE queue (url_id INTEGER PRIMARY KEY, seq INTEGER NOT NULL, depth INTEGER, priority INTEGER NOT NULL DEFAULT 0);
CREATE INDEX queue_order ON queue(priority, depth, seq);
CREATE TABLE list_input (position INTEGER PRIMARY KEY, original TEXT NOT NULL, url_id INTEGER, outcome TEXT NOT NULL);
  -- outcome: ok | duplicate | invalid | excluded | not_included | ignored_line

CREATE TABLE analysis_steps (step TEXT PRIMARY KEY, state TEXT NOT NULL, finished_at TEXT, config_hash TEXT) WITHOUT ROWID;
CREATE TABLE url_flags (url_id INTEGER NOT NULL, flag TEXT NOT NULL, value TEXT, PRIMARY KEY (url_id, flag)) WITHOUT ROWID;
CREATE INDEX url_flags_flag ON url_flags(flag);
  -- M1-Flags: non_indexable_canonical (TECH-018), in_sitemap (TECH-212), orphan + value=found_via_url_id (TECH-214),
  --           redirect_loop (CRAWL-017), redirect_limit_reached (CRAWL-072)
CREATE TABLE crawl_events (id INTEGER PRIMARY KEY, at TEXT NOT NULL, kind TEXT NOT NULL, code TEXT NOT NULL, message TEXT NOT NULL);
CREATE TABLE crawl_counters (key TEXT PRIMARY KEY, value INTEGER NOT NULL) WITHOUT ROWID;
  -- z. B. excluded_skipped, include_hits:<n>, robots_blocked_internal, robots_blocked_external, regex_too_complex:<rule>
CREATE TABLE page_html (url_id INTEGER PRIMARY KEY, encoding TEXT, body BLOB);   -- Vorhalt (CRAWL-063, M2), in M1 leer
