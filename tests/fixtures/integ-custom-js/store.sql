INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,content_class,content_type,status_code,status)
VALUES (1,'https://extract.test/a','https://extract.test/a','https://extract.test',1,1,'done','link',0,22,'HTML','text/html',200,'OK'),
(2,'https://extract.test/b','https://extract.test/b','https://extract.test',1,1,'done','link',0,22,'HTML','text/html',200,'OK'),
(3,'https://extract.test/c','https://extract.test/c','https://extract.test',1,1,'done','link',0,22,'HTML','text/html',200,'OK'),
(4,'https://extract.test/error','https://extract.test/error','https://extract.test',1,1,'done','link',0,26,'HTML','text/html',404,'Not Found'),
(5,'https://external.test/','https://external.test/','https://external.test',0,1,'done','link',0,22,'HTML','text/html',200,'OK'),
(6,'https://extract.test/pdf','https://extract.test/pdf','https://extract.test',1,1,'done','link',0,24,'PDF','application/pdf',200,'OK');
INSERT INTO links(source_id,target_id,target_raw,link_type,position,anchor,follow)
VALUES (2,1,'/a','Hyperlink',0,'to A',1),(2,1,'/a','Hyperlink',1,'again',0),(1,2,'/b','Hyperlink',0,'to B',1);
