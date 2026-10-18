-- F05-Store-Vertrag: Dateigrößen sind bereits entpackt, counts zählen XML-Elemente vor Deduplizierung.
INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,status_code,content_class,indexability,indexability_status)
VALUES
(1,'https://site.test/','https://site.test/','https://site.test',1,1,'done','start',0,18,200,'HTML','Indexable',''),
(2,'https://site.test/both','https://site.test/both','https://site.test',1,1,'done','link',0,22,200,'HTML','Indexable',''),
(3,'https://site.test/dup-same-file','https://site.test/dup-same-file','https://site.test',1,1,'done','link',0,30,200,'HTML','Indexable',''),
(4,'https://site.test/orphan','https://site.test/orphan','https://site.test',1,1,'done','sitemap',0,24,200,'HTML','Indexable',''),
(5,'https://site.test/orphan-child','https://site.test/orphan-child','https://site.test',1,1,'done','link',0,30,200,'HTML','Indexable',''),
(6,'https://site.test/noindex','https://site.test/noindex','https://site.test',1,1,'done','link',0,24,200,'HTML','Non-Indexable','noindex'),
(7,'https://site.test/not-in','https://site.test/not-in','https://site.test',1,1,'done','link',0,23,200,'HTML','Indexable',''),
(8,'https://other.test/external','https://other.test/external','https://other.test',0,1,'done','link',0,28,200,'HTML','Indexable',''),
(9,'https://site.test/pending','https://site.test/pending','https://site.test',1,1,'queued','sitemap',0,24,NULL,'HTML',NULL,NULL);
INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,status_code,content_class,content_type,size_bytes)
VALUES
(101,'https://site.test/a.xml','https://site.test/a.xml','https://site.test',1,0,'done','sitemap',0,24,200,'Other','application/xml',100),
(102,'https://site.test/b.xml.gz','https://site.test/b.xml.gz','https://site.test',1,0,'done','sitemap',0,27,200,'Other','application/gzip',100),
(103,'https://site.test/index.xml','https://site.test/index.xml','https://site.test',1,0,'done','sitemap',0,28,200,'Other','application/xml',100),
(104,'https://site.test/exact.xml','https://site.test/exact.xml','https://site.test',1,0,'done','sitemap',0,28,200,'Other','application/xml',100),
(105,'https://site.test/huge.xml.gz','https://site.test/huge.xml.gz','https://site.test',1,0,'done','sitemap',0,30,200,'Other','application/gzip',100),
(106,'https://site.test/invalid.xml','https://site.test/invalid.xml','https://site.test',1,0,'done','sitemap',0,30,200,'Other','application/xml',100);
INSERT INTO sitemaps(id,url,url_id,kind,entries,size_bytes,depth,found_via) VALUES
(1,'https://site.test/a.xml',101,'urlset',50001,100,0,'robots'),
(2,'https://site.test/b.xml.gz',102,'urlset',4,100,0,'robots'),
(3,'https://site.test/index.xml',103,'index',50001,52428801,0,'robots'),
(4,'https://site.test/exact.xml',104,'urlset',50000,52428800,0,'robots'),
(5,'https://site.test/huge.xml.gz',105,'urlset',1,52428801,0,'robots'),
(6,'https://site.test/invalid.xml',106,'invalid',50001,52428801,0,'robots');
INSERT INTO sitemap_entries(sitemap_id,url_id,loc_raw) VALUES
(1,2,'https://site.test/both'),(2,2,'https://site.test/both'),
(1,3,'https://site.test/dup-same-file'),
(1,4,'https://site.test/orphan'),(2,4,'https://site.test/orphan'),
(1,6,'https://site.test/noindex'),(1,9,'https://site.test/pending');
-- Doppelte <loc> in derselben Datei werden vom Store dedupliziert.
INSERT OR IGNORE INTO sitemap_entries(sitemap_id,url_id,loc_raw) VALUES (1,3,'https://site.test/dup-same-file');
-- Ein Eintrag eines Index zählt nicht als URL-Mitgliedschaft.
INSERT INTO sitemap_entries(sitemap_id,url_id,loc_raw) VALUES (3,3,'https://site.test/dup-same-file');
INSERT INTO links(source_id,target_id,target_raw,link_type,position,follow) VALUES
(1,2,'https://site.test/both','Hyperlink',0,1),
(1,3,'https://site.test/dup-same-file','Hyperlink',1,1),
(1,6,'https://site.test/noindex','Hyperlink',2,1),
(1,7,'https://site.test/not-in','Hyperlink',3,1),
(4,5,'https://site.test/orphan-child','Hyperlink',0,1);
