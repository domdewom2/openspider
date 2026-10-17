INSERT INTO urls (id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,status_code,status,title_1,title_count,content_type,content_class)
VALUES
(1,'https://site.test/b','https://site.test/b','https://site.test',1,'done','link',0,19,200,'OK','Beta',1,'text/html','HTML'),
(2,'https://site.test/a','https://site.test/a','https://site.test',1,'done','link',0,19,200,'OK',NULL,0,'text/html','HTML'),
(3,'https://site.test/c','https://site.test/c','https://site.test',1,'done','link',0,19,200,'OK','Gamma',1,'text/html','HTML');
INSERT INTO links(source_id,target_id,target_raw,link_type,position,follow)
VALUES (1,2,'/a','Hyperlink',1,1),(1,2,'/a','Hyperlink',2,1),
(2,1,'/b','Hyperlink',1,1),(2,1,'/b','Hyperlink',2,1),
(3,1,'/b','Hyperlink',1,1),(3,2,'/a','Hyperlink',2,1);
INSERT INTO list_input(position,original,url_id,outcome) VALUES
(1,'https://site.test/b',1,'ok'),(2,'https://site.test/a',2,'ok'),
(3,'https://site.test/b',1,'duplicate'),(4,'example.test/c',NULL,'ignored_line'),
(5,'https://[bad',NULL,'invalid'),(6,'https://site.test/private',NULL,'excluded');
