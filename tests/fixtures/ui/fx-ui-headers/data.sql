-- Twelve fetched URLs, plus one discovered but never fetched URL (excluded from report denominator).
WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<12)
INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,content_class,status_code,crawl_timestamp)
SELECT i,'https://www.site.test/'||printf('%02d',i),'https://www.site.test/'||printf('%02d',i),'https://www.site.test',1,1,'done','seed',0,25,'HTML',200,'2030-01-01T00:00:00.000Z' FROM n;
INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length)
VALUES(99,'https://www.site.test/queued','https://www.site.test/queued','https://www.site.test',1,1,'queued','link',0,29);
INSERT INTO http_headers(url_id,direction,position,name,value)
SELECT id,'response',0,CASE WHEN id%2=0 THEN 'Cache-Control' ELSE 'cache-control' END,CASE WHEN id<=8 THEN 'max-age=60' ELSE 'no-cache' END FROM urls WHERE id<=10;
INSERT INTO http_headers(url_id,direction,position,name,value)
SELECT id,'response',1,CASE WHEN id%2=0 THEN 'Content-Type' ELSE 'content-type' END,'text/html' FROM urls WHERE id<=12;
INSERT INTO http_headers VALUES (1,'response',2,'Set-Cookie','a=1'),(1,'response',3,'Set-Cookie','b=2'),(1,'response',4,'Link','</a>; rel=next'),(1,'response',5,'Link','</b>; rel=prev');
INSERT INTO http_headers VALUES (1,'request',0,'Authorization','Basic secret'),(1,'request',1,'pRoXy-AuThOrIzAtIoN','Proxy secret'),(1,'request',2,'Cookie','session=secret'),(1,'request',3,'Accept','text/html');
INSERT INTO cookies(url_id,position,origin,name,value,domain,path,max_age,secure,http_only,same_site,raw)
SELECT id,0,'http','shared','v'||id,'.site.test','/',86400,1,1,'lax','shared=value; Domain=.site.test; Max-Age=86400; Secure; HttpOnly; SameSite=Lax' FROM urls WHERE id<=12;
INSERT INTO cookies(url_id,position,origin,name,value,domain,path,max_age,secure,http_only,same_site,raw)
VALUES(1,1,'http','shared','another','.site.test','/',86400,1,1,'Lax','shared=another'),
(1,2,'http','session','1',NULL,NULL,NULL,1,1,'Strict','session=1; Secure; HttpOnly; SameSite=Strict'),
(1,3,'javascript','third','1','tracker.test','/',NULL,0,0,'none','third=1'),
(1,4,'http','third','2','tracker.test','/',NULL,0,0,'None','third=2'),
(1,5,'http','sibling','3','cdn.site.test','/',NULL,0,0,NULL,'sibling=3'),
(1,6,'http','trap','4','not-site.test','/',NULL,0,0,NULL,'trap=4');
INSERT INTO cookies(url_id,position,origin,name,value,domain,path,expires,secure,http_only,raw)
VALUES(1,7,'http','expires','5',NULL,NULL,'Wed, 02 Jan 2030 00:00:00 GMT',0,0,'expires=5');
