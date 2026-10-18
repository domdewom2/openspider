-- TECH-145: A=1 B=2 C=3 D=4 E=5 R=6 K=7; redirects/canonicals don't participate.
UPDATE urls SET status_code = 301, redirect_type = 'http', redirect_target_id = 3 WHERE id = 6;
UPDATE urls SET canonical_target_id = 4, canonical_self = 0 WHERE id = 7;
INSERT INTO links (source_id,target_id,target_raw,link_type,position,follow) VALUES
(1,2,'/2','Hyperlink',1,1),(1,6,'/6','Hyperlink',2,1),
(1,4,'/4','Hyperlink',3,0),(1,1,'/1','Hyperlink',4,1),
(2,3,'/3','Hyperlink',1,1),(2,3,'/3','Hyperlink',2,1),(2,1,'/1','Hyperlink',3,1),
(3,1,'/1','Hyperlink',1,1),(3,7,'/7','Hyperlink',2,1),
(4,5,'/5','Hyperlink',1,1),(5,1,'/1','Hyperlink',1,1);
