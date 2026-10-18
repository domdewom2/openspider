-- Exactly 20 distinct, followed, non-self hyperlinks per source.
-- Slot zero from sources 2..hub+1 points to URL 1. All other targets
-- lie on a ring of N-1 URLs, excluding the hub (so its count is exact).
WITH edges AS (
  SELECT numbers.n+?1 AS source, s.n AS slot FROM numbers CROSS JOIN slots s
  WHERE numbers.n+?1<=?2
), targets AS (
  SELECT *, CASE WHEN slot=0 AND source BETWEEN 2 AND ?4+1 THEN 1
    ELSE 2+(source-1+slot)%(?3-1) END AS target FROM edges
)
INSERT INTO links(id,source_id,target_id,target_raw,link_type,position,anchor,
                  follow,path_type,link_origin,in_head)
SELECT (source-1)*20+slot+1,source,target,
  CASE target WHEN 1 THEN 'https://ui.test/'
    ELSE printf('https://ui.test/s%03d/p%07d',target%100,target) END,
  'Hyperlink',slot,'Link '||slot,1,'Absolute','HTML',0
FROM targets ORDER BY source,slot
