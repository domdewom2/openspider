-- N14: numeric link facts without the overflow-text join or a change_seq scan.
-- Metadata only: no crawl-wide index build/copy in this migration. Point writes
-- use the existing PRIMARY KEY(url_id,key_id); the old fact snapshot reads the
-- compact primary tree once, rather than once per key/difference direction.
-- On WITHOUT ROWID tables NOT INDEXED still permits secondary-index scans;
-- explicitly select the existing primary index instead.
CREATE VIEW analysis_link_facts AS
SELECT e.url_id,k.key,e.v_int
FROM url_ext_data e INDEXED BY sqlite_autoindex_url_ext_data_1 CROSS JOIN ext_keys k ON k.id=e.key_id
WHERE e.key_id IN (SELECT id FROM ext_keys WHERE key IN
    ('link.mixed_inlinks','link.nofollow_only','link.non_indexable_only'));
