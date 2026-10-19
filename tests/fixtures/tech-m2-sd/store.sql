INSERT INTO urls (id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,status_code,content_class,indexability) VALUES
(1,'https://site.test/mixed','https://site.test/mixed','https://site.test',1,'Done','Seed',0,23,200,'HTML','Indexable'),
(2,'https://site.test/error','https://site.test/error','https://site.test',1,'Done','Seed',0,23,200,'HTML','Indexable'),
(3,'https://site.test/noindex','https://site.test/noindex','https://site.test',1,'Done','Seed',0,23,200,'HTML','Non-Indexable'),
(4,'https://site.test/micro','https://site.test/micro','https://site.test',1,'Done','Seed',0,23,200,'HTML','Indexable'),
(5,'https://site.test/external','https://site.test/external','https://site.test',0,'Done','Seed',0,23,200,'HTML','Indexable'),
(6,'https://site.test/created','https://site.test/created','https://site.test',1,'Done','Seed',0,23,201,'HTML','Indexable'),
(7,'https://site.test/image','https://site.test/image','https://site.test',1,'Done','Seed',0,23,200,'Image','Indexable'),
(8,'https://site.test/none','https://site.test/none','https://site.test',1,'Done','Seed',0,23,200,'HTML','Indexable'),
(9,'https://site.test/bad-noindex','https://site.test/bad-noindex','https://site.test',1,'Done','Seed',0,23,200,'HTML','Non-Indexable');
INSERT INTO sd_items(id,url_id,format,type_path,json) VALUES
(1,1,'json-ld','Product|Thing','{"@type":["https://schema.org/Product","https://schema.org/Thing"],"name":"X","offers":{"@type":"https://schema.org/Offer","price":"5"},"_os_order":["@type","name","offers"]}'),
(2,1,'json-ld','Product','{"@type":"https://schema.org/Product","name":"Y"}'),
(3,1,'microdata','Recipe','{"@type":["https://schema.org/Recipe"],"name":"Soup","nutrition":{"@type":["https://schema.org/NutritionInformation"]}}'),
(4,1,'rdfa','Organization','{"@type":["https://schema.org/Organization"],"name":"O"}'),
(5,2,'json-ld','','{"_os_parse_error":{"message":"Trailing comma","line":2,"column":9,"snippet":"bad,}"}}'),
(6,4,'microdata','Product','{"@type":["https://schema.org/Product"]}'),
(7,9,'json-ld','','{"_os_parse_error":{"message":"Empty block","line":1,"column":1,"snippet":""}}');
INSERT INTO sd_issues(url_id,item_id,validation,severity,code) VALUES
(2,5,'parse','error','Trailing comma (line 2, column 9)'),
(9,7,'parse','error','Empty block (line 1, column 1)');
