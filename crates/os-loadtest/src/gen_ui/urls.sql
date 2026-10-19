-- Three parameters: first ID, last ID, bounded seed. No crawl-wide Rust allocation.
WITH ids AS (SELECT n+?1 AS id FROM numbers WHERE n+?1<=?2),
facts AS MATERIALIZED (
  SELECT id, (id*1103515245+?3)%2147483647 AS r,
    CASE WHEN id=1 THEN 'https://ui.test/'
         ELSE printf('https://ui.test/s%03d/p%07d', id%100, id) END AS address
  FROM ids
),
texts AS MATERIALIZED (
  SELECT *, CASE r%17 WHEN 0 THEN NULL WHEN 1 THEN ''
    WHEN 2 THEN 'Äpfel 東京 café ' || (r%100)
    WHEN 3 THEN 'Duplicate title ' || (r%10)
    WHEN 4 THEN printf('%0300d',r)
    ELSE 'Page ' || r END AS title,
    'Description ' || (r%10000) AS description,
    'Heading ' || (r%1000) AS heading
  FROM facts
)
INSERT INTO urls (
  id,url,address,origin,is_internal,in_tables,state,discovered_via,
  crawl_depth,folder_depth,url_length,crawl_timestamp,status_code,status,
  content_type,content_class,http_version,size_bytes,transferred_bytes,
  total_transferred_bytes,response_time_ms,last_modified_raw,last_modified,hash_md5,charset,
  title_count,title_1,title_1_len,title_1_px,title_1_in_head,
  title_2,title_2_len,title_2_px,title_2_in_head,
  meta_desc_count,meta_desc_1,meta_desc_1_len,meta_desc_1_px,meta_desc_1_in_head,
  meta_desc_2,meta_desc_2_len,meta_desc_2_px,meta_desc_2_in_head,
  meta_kw_count,meta_kw_1,meta_kw_1_len,meta_kw_2,meta_kw_2_len,
  h1_count,h1_1,h1_1_len,h1_2,h1_2_len,h2_count,h2_1,h2_1_len,h2_2,h2_2_len,
  word_count,text_ratio,canonical_count,canonical_url,canonical_target_id,canonical_self,
  effective_index,effective_follow,has_rel_next,has_rel_prev,hreflang_count,
  indexability,indexability_status,change_seq
)
SELECT id,address,address,'https://ui.test',1,1,'done',CASE id WHEN 1 THEN 'start' ELSE 'link' END,
  CASE id WHEN 1 THEN 0 ELSE 1+r%4 END,CASE id WHEN 1 THEN 0 ELSE 2 END,length(address),
  '2025-10-12T00:00:00.000Z',CASE WHEN id%100=0 THEN 404 ELSE 200 END,
  CASE WHEN id%100=0 THEN 'Not Found' ELSE 'OK' END,
  'text/html; charset=UTF-8','HTML','HTTP/2',
  CASE id WHEN 2 THEN 15728640 ELSE 1000+r%200000 END,
  500+r%80000,500+r%80000,r%5000,
  'Sat, 11 Oct 2025 00:00:00 GMT','2025-10-11T00:00:00.000Z',printf('%032x',r%10000),'UTF-8',
  CASE WHEN title IS NULL THEN 0 WHEN r%7=0 THEN 2 ELSE 1 END,
  title,length(title),length(title)*7,1,
  CASE WHEN r%7=0 AND title IS NOT NULL THEN title END,
  CASE WHEN r%7=0 AND title IS NOT NULL THEN length(title) END,
  CASE WHEN r%7=0 AND title IS NOT NULL THEN length(title)*7 END,
  CASE WHEN r%7=0 AND title IS NOT NULL THEN 1 END,
  1,description,length(description),length(description)*7,1,NULL,NULL,NULL,NULL,
  1,'benchmark',9,NULL,NULL,
  1,heading,length(heading),NULL,NULL,1,heading,length(heading),NULL,NULL,
  CASE r%19 WHEN 0 THEN NULL WHEN 1 THEN 0 WHEN 2 THEN 2 WHEN 3 THEN 10 ELSE r%10000 END,
  (r%1000)/10.0,0,NULL,NULL,NULL,
  r%13!=0,1,0,0,0,
  CASE WHEN id%100=0 OR r%13=0 THEN 'Non-Indexable' ELSE 'Indexable' END,
  CASE WHEN id%100=0 THEN 'Client Error' WHEN r%13=0 THEN 'noindex' END,1
FROM texts ORDER BY id
