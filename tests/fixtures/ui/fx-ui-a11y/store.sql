INSERT INTO urls(id,url,address,is_internal,in_tables,content_class,status_code,state,origin,discovered_via,folder_depth,url_length) VALUES
(1,'https://a.test/one','https://a.test/one',1,1,'HTML',200,'done','https://a.test','seed',0,20),
(2,'https://a.test/two','https://a.test/two',1,1,'HTML',200,'done','https://a.test','seed',0,20),
(3,'https://a.test/three','https://a.test/three',1,1,'HTML',404,'done','https://a.test','seed',0,20),
(4,'https://a.test/four','https://a.test/four',0,1,'HTML',200,'done','https://a.test','seed',0,20),
(5,'https://a.test/five','https://a.test/five',1,1,'HTML',200,'done','https://a.test','seed',0,20);
INSERT INTO a11y_rules VALUES (1,'image-alt','violation','critical'),(1,'heading-order','violation','moderate'),(1,'empty-heading','violation','minor'),(1,'color-contrast','incomplete','serious'),(1,'html-has-lang','pass',NULL),(1,'video-caption','inapplicable',NULL),(2,'image-alt','violation','critical'),(3,'image-alt','violation','critical'),(4,'image-alt','violation','critical');
INSERT INTO a11y_nodes(url_id,rule_id,target,snippet,summary) VALUES
(1,'image-alt','#a','<img id="a">','{}'),(1,'image-alt','#b','<img id="b">','{}'),(1,'heading-order','h3','<h3>Heading</h3>','{}'),(1,'empty-heading','h2','<h2></h2>','{}'),(1,'color-contrast','p','<p>Background image</p>','{}'),(2,'image-alt','#c','<img id="c">','{}');
