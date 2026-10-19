INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(1,'https://site.test/','https://site.test/','https://site.test',1,'done','start',0,0,30,200,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(2,'https://site.test/a','https://site.test/a','https://site.test',1,'done','link',1,0,30,200,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(3,'https://site.test/a/b','https://site.test/a/b','https://site.test',1,'done','link',2,0,30,200,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(4,'https://site.test/a/b/c','https://site.test/a/b/c','https://site.test',1,'done','link',3,0,30,200,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(5,'https://site.test/redirect','https://site.test/redirect','https://site.test',1,'done','link',1,0,30,301,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(6,'https://site.test/z','https://site.test/z','https://site.test',1,'done','redirect',2,0,30,200,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(7,'https://site.test/l','https://site.test/l','https://site.test',1,'done','link',1,0,30,200,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(8,'https://site.test/r','https://site.test/r','https://site.test',1,'done','link',1,0,30,200,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(9,'https://site.test/diamond','https://site.test/diamond','https://site.test',1,'done','link',1,0,30,200,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(10,'https://site.test/sitemap.xml','https://site.test/sitemap.xml','https://site.test',1,'done','sitemap',0,0,30,200,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(11,'https://site.test/orphan','https://site.test/orphan','https://site.test',1,'done','sitemap',1,0,30,200,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(12,'https://site.test/api','https://site.test/api','https://site.test',1,'discovered','link',NULL,0,30,NULL,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(13,'https://site.test/list','https://site.test/list','https://site.test',1,'done','list_input',0,0,30,200,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(14,'https://site.test/list-z','https://site.test/list-z','https://site.test',1,'done','redirect',1,0,30,200,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(15,'https://site.test/queued','https://site.test/queued','https://site.test',1,'queued','link',1,0,30,NULL,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(16,'https://site.test/done','https://site.test/done','https://site.test',1,'done','link',1,0,30,200,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(17,'https://site.test/excluded','https://site.test/excluded','https://site.test',1,'queued','link',1,0,30,NULL,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(18,'https://site.test/blocked','https://site.test/blocked','https://site.test',1,'queued','link',1,0,30,NULL,'HTML','Indexable');
INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability) VALUES(19,'https://site.test/external','https://site.test/external','https://site.test',0,'queued','link',1,0,30,NULL,'HTML','Indexable');
INSERT INTO discovery_edges(seq,target_id,source_id,kind,link_type,anchor,depth) VALUES
(1,1,NULL,'start',NULL,NULL,0),
(2,2,1,'link','Hyperlink','A',1),
(3,3,2,'link','Hyperlink','B',2),
(4,4,3,'link','Hyperlink','C',3),
(5,5,1,'link','Hyperlink','redirect',1),
(6,6,5,'redirect','HTTP Redirect',NULL,2),
(7,7,1,'link','Hyperlink','left',1),
(8,8,1,'link','Hyperlink','right',1),
(9,9,7,'link','Hyperlink','diamond',2),
(10,9,8,'link','Hyperlink','later',2),
(11,11,NULL,'sitemap',NULL,NULL,1),
(13,13,NULL,'list_input',NULL,NULL,0),
(14,14,13,'redirect','HTTP Redirect',NULL,1),
(15,15,1,'link','Hyperlink','queued',1),
(99,9,1,'link','Hyperlink','shorter later',1);
INSERT INTO sitemaps(id,url,url_id,kind,depth,found_via) VALUES(1,'https://site.test/sitemap.xml',10,'urlset',0,'manual');
INSERT INTO sitemap_entries(sitemap_id,url_id,loc_raw) VALUES(1,11,'https://site.test/orphan');
INSERT INTO queue(url_id,seq,depth) VALUES(15,15,1),(16,16,1),(17,17,1),(18,18,1),(19,19,1);
UPDATE urls SET not_crawled_reason='excluded' WHERE id=17;
UPDATE urls SET robots_blocked=1 WHERE id=18;
INSERT INTO links(id,source_id,target_id,target_raw,link_type,position,anchor,follow) VALUES(1,1,2,'/a','Hyperlink',0,'A',1),(2,2,3,'/a/b','Hyperlink',0,'B',1),(3,3,4,'/a/b/c','Hyperlink',0,'C',1),(4,1,7,'/l','Hyperlink',1,'left',1),(5,1,8,'/r','Hyperlink',2,'right',1),(6,7,9,'/diamond','Hyperlink',0,'diamond',1),(7,8,9,'/diamond','Hyperlink',0,'later',1),(8,9,1,'/','Hyperlink',0,'cycle',1);
