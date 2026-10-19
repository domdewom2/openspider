-- OUT-121/122: Store-Vertrag, unabhängige Ausschlussgründe einschließlich Kombinationen.
WITH p(id, path, status, class, idx, reason, effective, canon, prev) AS (VALUES
 (1,'/',200,'HTML','Indexable',NULL,1,NULL,0),
 (2,'/noindex',200,'HTML','Non-Indexable','noindex',0,NULL,0),
 (3,'/canonical',200,'HTML','Non-Indexable','Canonicalised',1,'https://site.test/',0),
 (4,'/paginated',200,'HTML','Indexable',NULL,1,NULL,1),
 (5,'/doc.pdf',200,'PDF','Indexable',NULL,1,NULL,0),
 (6,'/redirect',301,'Unknown','Non-Indexable','Redirected',1,NULL,0),
 (7,'/missing',404,'HTML','Non-Indexable','Client Error',1,NULL,0),
 (8,'/error',503,'HTML','Non-Indexable','Server Error',1,NULL,0),
 (9,'/no-response',0,'Unknown','Non-Indexable','No Response',1,NULL,0),
 (10,'/both',200,'HTML','Non-Indexable','noindex',0,'https://site.test/',0),
 (11,'/pdf-noindex',200,'PDF','Non-Indexable','noindex',0,NULL,0),
 (12,'/impressum',200,'HTML','Indexable',NULL,1,NULL,0),
 (13,'/search?sessionid=secret',200,'HTML','Indexable',NULL,1,NULL,0),
 (14,'/robots',200,'HTML','Non-Indexable','Blocked by Robots.txt',1,NULL,0),
 (15,'/removed',200,'HTML','Indexable',NULL,1,NULL,0),
 (16,'/not-crawled',NULL,'HTML',NULL,NULL,1,NULL,0),
 (17,'/external',200,'HTML','Indexable',NULL,1,NULL,0),
 (18,'/style.css',200,'CSS','Indexable',NULL,1,NULL,0),
 (19,'/refresh',200,'HTML','Non-Indexable','Self Referencing Meta Refresh',1,NULL,0),
 (20,'/redirect-noindex',302,'HTML','Non-Indexable','Redirected',0,NULL,0),
 (21,'/redirect-canonical',302,'HTML','Non-Indexable','Redirected',1,'https://site.test/',0)
)
INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability,indexability_status,effective_index,canonical_url,canonical_self,has_rel_prev,robots_blocked)
SELECT id,'https://site.test'||path,'https://site.test'||path,'https://site.test',id!=17,id!=15,
 CASE WHEN id=16 THEN 'discovered' ELSE 'done' END,'link',0,0,30,status,class,idx,reason,effective,canon,
 CASE WHEN canon IS NULL THEN 1 ELSE 0 END,prev,id=14 FROM p;
