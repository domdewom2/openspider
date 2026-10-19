-- OUT-123/124/125: feste Crawlzeit, Tiefen 0..7, jung/alt/24h/fehlend/ungültig/zukünftig.
WITH p(id,depth,modified) AS (VALUES
 (1,0,'2026-10-01T10:00:00.000Z'),
 (2,1,'2026-10-13T11:00:00.000Z'),
 (3,2,'2026-10-11T12:00:00.000Z'),
 (4,3,'2026-10-12T12:00:00.000Z'),
 (5,4,NULL),
 (6,5,'invalid'),
 (7,6,'2026-10-14T12:00:00.000Z'),
 (8,7,'2026-10-13T12:00:00.000Z')
)
INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability,effective_index,last_modified,crawl_timestamp)
SELECT id,'https://site.test/p'||depth,'https://site.test/p'||depth,'https://site.test',1,1,'done','link',depth,0,30,200,'HTML','Indexable',1,modified,'2026-10-13T12:00:00.000Z' FROM p;
