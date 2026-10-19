-- OUT-128: HTML/HTTP einschließlich Selbstverweis; kaputte, unbekannte und nicht indexierbare Ziele.
WITH p(id,path,status,idx) AS (VALUES
 (1,'/de/',200,'Indexable'),(2,'/en/',200,'Indexable'),(3,'/',200,'Indexable'),
 (4,'/missing/',404,'Non-Indexable'),(5,'/noindex/',200,'Non-Indexable')
)
INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability,effective_index)
SELECT id,'https://site.test'||path,'https://site.test'||path,'https://site.test',1,1,'done','link',0,0,30,status,'HTML',idx,id!=5 FROM p;
INSERT INTO hreflang(url_id,source,position,code_raw,code_norm,code_valid,href_raw,url,target_id,in_head) VALUES
 (1,'html',0,'de','de',1,'/de/','https://site.test/de/',1,1),
 (1,'html',1,'en','en',1,'/en/','https://site.test/en/',2,1),
 (1,'http',0,'x-default','x-default',1,'/','https://site.test/',3,1),
 (1,'http',1,'fr','fr',1,'/missing/','https://site.test/missing/',4,1),
 (1,'http',2,'es','es',1,'/noindex/','https://site.test/noindex/',5,1),
 (1,'http',3,'it','it',1,'/unknown/','https://site.test/unknown/',NULL,1),
 (1,'sitemap',0,'nl','nl',1,'/en/','https://site.test/en/',2,1);
