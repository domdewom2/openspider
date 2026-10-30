-- OUT-085: IDs deliberately differ from upload order; zero, one and two redirects.
INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,status,content_class,content_type,indexability,effective_index,crawl_timestamp)
SELECT 1,'https://site.test/start','https://site.test/start','https://site.test',1,1,'done','list_input',0,0,23,301,'Moved Permanently','HTML','text/html','Non-Indexable',1,'2026-10-13T12:00:00.000Z'
UNION ALL SELECT 2,'https://site.test/middle','https://site.test/middle','https://site.test',1,1,'done','redirect',0,0,24,302,'Found','HTML','text/html','Non-Indexable',1,'2026-10-13T12:00:00.000Z'
UNION ALL SELECT 3,'https://site.test/end','https://site.test/end','https://site.test',1,1,'done','redirect',0,0,21,200,'OK','HTML','text/html','Indexable',1,'2026-10-13T12:00:00.000Z';
UPDATE urls SET redirect_type='http',redirect_url='https://site.test/middle',redirect_target_id=2 WHERE id=1;
UPDATE urls SET redirect_type='http',redirect_url='https://site.test/end',redirect_target_id=3 WHERE id=2;
INSERT INTO list_input(position,original,url_id,outcome) VALUES (0,'https://site.test/end',3,'ok'),(1,'https://site.test/start',1,'ok'),(2,'https://site.test/middle',2,'ok');
