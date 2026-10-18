-- OUT-126/127: 20 einbindende Seiten, Produktbild, Logo, Grenzen 5/6, CDN, 404, doppelte Instanzen.
WITH RECURSIVE p(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM p WHERE i<20)
INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability,effective_index)
SELECT i,'https://site.test/p'||i,'https://site.test/p'||i,'https://site.test',1,1,'done','link',1,0,30,200,'HTML','Indexable',1 FROM p;
WITH p(id,path,status,internal) AS (VALUES
 (101,'https://site.test/product.jpg',200,1),
 (102,'https://site.test/logo.jpg',200,1),
 (103,'https://site.test/five.jpg',200,1),
 (104,'https://site.test/six.jpg',200,1),
 (105,'https://cdn.test/p.jpg?a=1&b=2',200,0),
 (106,'https://site.test/missing.jpg',404,1),
 (107,'https://other.test/p.jpg',200,0)
)
INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,crawl_depth,folder_depth,url_length,status_code,content_class,indexability,effective_index)
SELECT id,path,path,'https://site.test',internal,1,'done','link',1,0,30,status,'Image','Indexable',1 FROM p;
INSERT INTO image_instances(page_id,image_id,src_raw,source,alt_present,position)
SELECT id,102,'/logo.jpg','img',0,1 FROM urls WHERE id<=20;
INSERT INTO image_instances(page_id,image_id,src_raw,source,alt_present,position)
SELECT id,103,'/five.jpg','img',0,2 FROM urls WHERE id<=5;
INSERT INTO image_instances(page_id,image_id,src_raw,source,alt_present,position)
SELECT id,104,'/six.jpg','img',0,3 FROM urls WHERE id<=6;
INSERT INTO image_instances(page_id,image_id,src_raw,source,alt_present,position) VALUES
 (1,101,'/product.jpg','img',0,4),(1,101,'/product.jpg','img',0,5),
 (1,105,'https://cdn.test/p.jpg?a=1&b=2','img',0,6),
 (1,106,'/missing.jpg','img',0,7),(1,107,'https://other.test/p.jpg','img',0,8);
UPDATE url_counters SET img_inlinks = (SELECT COUNT(DISTINCT page_id) FROM image_instances WHERE image_id=url_id);
