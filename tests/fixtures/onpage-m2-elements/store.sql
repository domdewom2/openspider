INSERT INTO urls (id, url, address, origin, is_internal, state, discovered_via, folder_depth, url_length, status_code, content_class, in_tables, indexability, title_1, title_1_px, meta_desc_1_px, h1_1, meta_kw_1, meta_kw_count) VALUES
(1,'https://site.test/1','https://site.test/1','https://site.test',1,'done','link',0,19,200,'HTML',1,'Indexable','Kontakt',561,985,'Kontakt','a, b',1),
(2,'https://site.test/2','https://site.test/2','https://site.test',1,'done','link',0,19,200,'HTML',1,'Indexable','kontakt',562,986,'Kontakt','A, B',3),
(3,'https://site.test/3','https://site.test/3','https://site.test',1,'done','link',0,19,200,'HTML',1,'Indexable','Start',199,399,'Other','a,b',1),
(4,'https://site.test/4','https://site.test/4','https://site.test',1,'done','link',0,19,200,'HTML',1,'Indexable','',0,0,'','',0),
(5,'https://site.test/5','https://site.test/5','https://site.test',1,'done','link',0,19,200,'HTML',1,'Indexable','Exact',200,400,'Other',NULL,1);
INSERT INTO url_ext (url_id,key,v_int) VALUES (1,'validation.checked',1),(1,'head.title_outside',1),(1,'head.description_outside',1),(1,'head.h1_alt',1),(1,'head.h1_non_sequential',1),(1,'head.h2_non_sequential',1);
