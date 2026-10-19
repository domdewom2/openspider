-- Store fixture fx-ui-basic: stable URLs, duplicate titles, missing description,
-- redirect, error, image without alt, external/hidden URL, and duplicate links/headers.
INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,content_class,status_code,status,indexability,indexability_status,crawl_depth,title_1,title_1_len,title_count,meta_desc_count,word_count,http_version,response_time_ms,canonical_self,effective_index) VALUES
(1,'https://site.test/','https://site.test/','https://site.test',1,1,'done','seed',0,18,'HTML',200,'OK','Indexable',NULL,0,'Same title',10,1,0,100,'HTTP/2',512,1,1),
(2,'https://site.test/blog/a','https://site.test/blog/a','https://site.test',1,1,'done','link',1,24,'HTML',200,'OK','Indexable',NULL,1,'Same title',10,1,0,200,'HTTP/2',250,1,1),
(3,'https://site.test/old','https://site.test/old','https://site.test',1,1,'done','link',0,21,'HTML',301,'Moved Permanently','Non-Indexable','Redirected',1,NULL,NULL,NULL,NULL,NULL,'HTTP/1.1',100,NULL,NULL),
(4,'https://site.test/missing','https://site.test/missing','https://site.test',1,1,'done','link',0,25,'HTML',404,'Not Found','Non-Indexable','Client Error',1,NULL,NULL,NULL,NULL,NULL,'HTTP/2',125,NULL,NULL),
(5,'https://site.test/image.png','https://site.test/image.png','https://site.test',1,1,'done','link',0,26,'Images',200,'OK','Indexable',NULL,1,NULL,NULL,NULL,NULL,NULL,'HTTP/2',250,NULL,NULL),
(6,'https://external.test/','https://external.test/','https://external.test',0,1,'discovered','link',0,22,'Unknown',NULL,NULL,NULL,NULL,1,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL),
(7,'https://site.test/private','https://site.test/private','https://site.test',1,0,'discovered','link',0,25,'Unknown',NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL),
(8,'https://site.test/noindex','https://site.test/noindex','https://site.test',1,1,'done','link',0,25,'HTML',200,'OK','Non-Indexable','Noindex',1,'Hidden',6,1,0,50,'HTTP/2',125,1,0);
UPDATE urls SET robots_blocked=1,not_crawled_reason='crawl_disabled' WHERE id=7;
UPDATE urls SET redirect_url='https://site.test/blog/a',redirect_target_id=2,redirect_type='http' WHERE id=3;
INSERT INTO links(id,source_id,target_id,target_raw,link_type,position,anchor,follow,rel,target_attr,path_type,link_path,link_position,link_origin,crawlable,uncrawlable_reason) VALUES
(1,1,2,'/blog/a?sid=1','Hyperlink',0,' Alpha ',1,NULL,'_blank','Root-Relative','/html[1]/body[1]/main[1]/a[1]','Content','HTML',1,NULL),
(2,1,2,'/blog/a','Hyperlink',1,'Alpha',1,NULL,NULL,'Root-Relative','/html[1]/body[1]/nav[1]/a[1]','Nav','HTML',1,NULL),
(3,1,2,'/blog/a','Hyperlink',2,'Beta',0,'nofollow',NULL,'Root-Relative','/html[1]/body[1]/main[1]/a[2]','Content','HTML',1,NULL),
(4,2,2,'/blog/a','Hyperlink',0,'Self',1,NULL,NULL,'Root-Relative','/html[1]/body[1]/a[1]','Content','HTML',1,NULL),
(5,1,4,'/missing','Hyperlink',3,'Missing',1,NULL,NULL,'Root-Relative',NULL,'Content','HTML',1,NULL),
(6,1,6,'https://external.test/','Hyperlink',4,'External',1,NULL,'_blank','Absolute',NULL,'Content','HTML',1,NULL),
(7,1,5,'/image.png','Image',5,NULL,1,NULL,NULL,'Root-Relative',NULL,'Content','HTML',1,NULL),
(8,1,NULL,'/script-only','Hyperlink',6,'Script',1,NULL,NULL,'Root-Relative',NULL,'Content','HTML',0,'non_anchor_href'),
(9,3,2,'https://site.test/blog/a','HTTP Redirect',0,NULL,1,NULL,NULL,'Absolute',NULL,NULL,'HTML',1,NULL);
INSERT INTO http_headers(url_id,direction,position,name,value) VALUES
(1,'request',0,'Authorization','Bearer should-never-escape'),
(1,'response',0,'Set-Cookie','a=1'),(1,'response',1,'Set-Cookie','b=2'),
(1,'response',2,'X-Fixture','ok'),(3,'response',0,'Location','/blog/a');
INSERT INTO directives(url_id,source,position,raw,tokens,applies,in_head) VALUES
(1,'meta_robots',0,'max-snippet:50','max-snippet:50',1,1),
(1,'meta_robots',1,'max-snippet:10','max-snippet:10',1,1);
INSERT INTO image_instances(page_id,image_id,src_raw,source,alt_present,position) VALUES(1,5,'/image.png','img',0,0);
UPDATE crawl_meta SET value='42' WHERE key='change_seq';
INSERT OR REPLACE INTO crawl_meta(key,value) VALUES('started_at','2026-10-13T09:30:00Z');
