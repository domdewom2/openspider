-- Closed-form counts for links.sql, O(N) instead of sorting 100 Mio. links.
-- Sources 1 and N share the same ring origin: targets 2..21 gain one.
-- Redirecting slot zero to the hub removes one from targets 3..hub+2,
-- and also target 2 if every non-hub source links to the hub.
WITH ids AS (SELECT n+?1 AS id FROM numbers WHERE n+?1<=?2),
counts AS (
  SELECT id, CASE WHEN id=1 THEN ?4 ELSE
    20+(id BETWEEN 2 AND 21)-(id BETWEEN 3 AND ?4+2)-(?4=?3-1 AND id=2)
    END AS incoming FROM ids
), errors AS (
  -- One in every 100 sources is a 404. Count multiples of 100 in the
  -- twenty-source ring interval, wrapping below source 2 back to N.
  SELECT *, CASE WHEN id=1 THEN (?4+1)/100 ELSE
    (id-1)/100 - CASE WHEN id>=22 THEN (id-21)/100
      ELSE -(?3/100-(?3+id-22)/100) END
    - (id BETWEEN 3 AND ?4+2 AND (id-1)%100=0)
    - (?4=?3-1 AND id=2 AND ?3%100=0)
    END AS error_sources FROM counts
)
INSERT INTO url_counters(url_id,inlinks,unique_inlinks,inlinks_all,html_2xx_sources,
                         outlinks,unique_outlinks,change_seq)
SELECT id,incoming,incoming,incoming,incoming-error_sources,20,20,1 FROM errors ORDER BY id
