-- Store fixture for hreflang/, out-hreflang and outside-head from TECH-075–089 / OUT-090–096.
INSERT INTO urls (id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,status_code,content_class,effective_index,indexability)
SELECT value, 'https://site.test/' || value, 'https://site.test/' || value,
       'https://site.test', 1, 'done', 'hreflang', 1, 24, 200, 'HTML', 1, 'Indexable'
FROM json_each('[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17]');
UPDATE urls SET status_code=404, status='Not Found', indexability='Non-Indexable', indexability_status='Client Error' WHERE id=4;
UPDATE urls SET state='queued', status_code=NULL, indexability=NULL WHERE id=5;
UPDATE urls SET effective_index=0, indexability='Non-Indexable', indexability_status='noindex', canonical_count=1, canonical_self=0, canonical_target_id=2, canonical_url='https://site.test/2' WHERE id=6;
UPDATE urls SET canonical_count=1, canonical_self=0, canonical_target_id=2, canonical_url='https://site.test/2', indexability='Non-Indexable', indexability_status='Canonicalised' WHERE id IN (7,10);
UPDATE urls SET is_internal=0, content_class='Unknown' WHERE id=8;
UPDATE urls SET content_class='PDF' WHERE id=9;
UPDATE urls SET canonical_count=1, canonical_self=1, canonical_target_id=id, canonical_url=url WHERE id=1;
UPDATE url_counters SET inlinks=1 WHERE url_id IN (1,2,6,7,9);
INSERT INTO hreflang (url_id,source,position,code_raw,code_norm,code_valid,href_raw,url,target_id,in_head) VALUES
(1,'html',0,'en-GB','en-gb',1,'/1','https://site.test/1',1,1),
(1,'html',1,'de','de',1,'/2','https://site.test/2',2,1),
(1,'html',2,'fr','fr',1,'/3','https://site.test/3',3,1),
(1,'html',3,'it','it',1,'/4','https://site.test/4',4,1),
(1,'html',4,'nl','nl',1,'/5','https://site.test/5',5,1),
(1,'html',5,'es','es',1,'/6','https://site.test/6',6,1),
(1,'html',6,'pt','pt',1,'/7','https://site.test/7',7,1),
(1,'html',7,'sv','sv',1,'/8','https://site.test/8',8,1),
(1,'html',8,'pl','pl',1,'/9','https://site.test/9',9,1),
(1,'html',9,'da','da',1,'/16','https://site.test/16',16,1),
(2,'sitemap',0,'EN-gb','en-gb',1,'/1','https://site.test/1',1,1),
(2,'http',0,'de','de',1,'/2','https://site.test/2',2,1),
(7,'html',0,'en','en',1,'/1','https://site.test/1',1,1),
(10,'html',0,'en','en',1,'/10','https://site.test/10',10,1),
(11,'http',0,'de','de',1,'/2','https://site.test/2',2,1),
(12,'html',0,'DE','de',1,'/2','https://site.test/2',2,1),
(12,'sitemap',0,'de','de',1,'/2','https://site.test/2',2,1),
(13,'html',0,'x-default','x-default',1,'/13','https://site.test/13',13,0),
(16,'http',0,'da','da',1,'/16','https://site.test/16',16,1),
(17,'html',0,'de','de',1,'/2','https://site.test/2',2,1);
INSERT INTO url_ext (url_id,key,v_int) VALUES (1,'validation.checked',1),(13,'validation.checked',1),(1,'hreflang.truncated',2),(1,'hreflang.truncated_sitemap',3);

-- SQ-04/TECH-075: real hyperlinks and independent outside-head evidence.
INSERT INTO links(source_id,target_id,target_raw,link_type,position,follow)
SELECT 17,value,'/'||value,'Hyperlink',value,1 FROM json_each('[1,2,6,7,9]');
INSERT INTO url_ext(url_id,key,v_int) VALUES (13,'hreflang.outside_head',1);
DELETE FROM hreflang WHERE url_id=13;
