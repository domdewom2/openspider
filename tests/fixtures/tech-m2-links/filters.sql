UPDATE urls SET indexability = 'Non-Indexable' WHERE id IN (2,3);
UPDATE urls SET is_internal = 0, content_class = 'Images', address = 'http://127.2.3.4/x' WHERE id = 12;
INSERT INTO links (source_id,target_id,target_raw,link_type,position,follow,anchor) VALUES
 (1,4,'/4','Hyperlink',1,1,'  Click   Here!  '),
 (2,4,'/4','Hyperlink',1,0,'Produkte'),
 (2,5,'/5','Hyperlink',2,0,'Mehr…'),
 (3,5,'/5','Hyperlink',1,0,'Read More!'),
 (2,6,'/6','Hyperlink',3,1,'Produkte'),
 (3,6,'/6','Hyperlink',2,1,'Produkte'),
 (6,6,'/6','Hyperlink',1,1,'Produkte'),
 (1,7,'/7','Hyperlink',2,1,'Hier klicken für Preise'),
 (1,12,'http://127.2.3.4/x','Image',3,1,NULL),
 (1,12,'http://127.2.3.4/x','Hyperlink',4,1,NULL),
 (1,12,'http://127.2.3.4/x','Hyperlink',5,0,NULL),
 (1,4,'/4','Hyperlink',6,1,'Produkte'),
 (7,4,'/4','Hyperlink',4,1,'Produkte'),
 (8,4,'/4','Hyperlink',4,1,'Produkte'),
 (3,4,'/4','Hyperlink',3,1,'produkte'),
 (3,4,'/4','Image',4,1,'ignored');
INSERT INTO links (source_id,target_id,target_raw,link_type,position,follow,anchor,alt_text) VALUES
 (1,4,'/4','Hyperlink',7,1,'  ', 'Logo'),
 (2,4,'/4','Hyperlink',5,1,NULL,NULL),
 (3,4,'/4','Hyperlink',5,1,' Mehr  ',NULL),
 (3,4,'/4','Hyperlink',6,1,'Mehr',NULL),
 (4,4,'/4','Hyperlink',1,1,'Produkte',NULL);
INSERT INTO links (source_id,target_id,target_raw,link_type,position,follow,crawlable,uncrawlable_reason) VALUES
 (8,9,'/9','Hyperlink',1,1,0,'non_anchor_href'),
 (8,NULL,'mailto:test@site.test','Hyperlink',2,1,0,'anchor_missing_href');
