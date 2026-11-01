#!/usr/bin/env python3
"""M2-N25: Durchsatz einer Crawl-Datei je 10-s-Fenster (nur lesend).

Aufruf: python3 n25-tail.py <crawl.ospider>

Zeigt je Fenster: abgeschlossene Abrufe, davon extern, neue Hosts (robots.txt-Abrufe), mittlere Antwortzeit
intern/extern und Bilder. Damit lässt sich entscheiden, ob ein Ratenabfall nach dem Fortsetzen an der Art der
verbleibenden URLs liegt (externe Prüfungen, neue Hosts, Bilder) oder an der App (gleiche Art, gleiche
Antwortzeiten, trotzdem weniger Abrufe je Fenster).
"""
import sqlite3, sys

con = sqlite3.connect(f"file:{sys.argv[1]}?mode=ro", uri=True)
rows = con.execute(
    """
    SELECT substr(crawl_timestamp, 1, 18) || '0' AS w,
           SUM(discovered_via <> 'robots_txt') AS n,
           SUM(discovered_via <> 'robots_txt' AND is_internal = 0) AS ext,
           SUM(discovered_via = 'robots_txt') AS hosts,
           ROUND(AVG(CASE WHEN is_internal = 1 AND discovered_via <> 'robots_txt' THEN response_time_ms END)) AS ms_int,
           ROUND(AVG(CASE WHEN is_internal = 0 AND discovered_via <> 'robots_txt' THEN response_time_ms END)) AS ms_ext,
           SUM(content_type LIKE 'image/%') AS img
    FROM urls WHERE crawl_timestamp IS NOT NULL
    GROUP BY w ORDER BY w
    """
).fetchall()
print(f"{'Fenster (UTC)':22} {'URLs':>5} {'extern':>6} {'neue Hosts':>10} {'ms int':>7} {'ms ext':>7} {'Bilder':>6}")
for w, n, ext, hosts, ms_int, ms_ext, img in rows:
    print(f"{w:22} {n or 0:5} {ext or 0:6} {hosts or 0:10} {ms_int if ms_int is not None else '-':>7} {ms_ext if ms_ext is not None else '-':>7} {img or 0:6}")

# Befund 4: Zeilen, die in „Gefundene URLs insgesamt“ zählen, aber in keiner Tabelle stehen (in_tables = 0)
print()
print("Nicht angezeigte URL-Zeilen (in_tables = 0) nach Herkunft und Grund:")
for via, reason, internal, n in con.execute(
    """
    SELECT discovered_via, COALESCE(not_crawled_reason, '-'), is_internal, COUNT(*)
    FROM urls WHERE in_tables = 0 GROUP BY 1, 2, 3 ORDER BY 4 DESC
    """
):
    print(f"  {via:16} {reason:24} {'intern' if internal else 'extern'} {n}")
total, shown = con.execute("SELECT COUNT(*), SUM(in_tables = 1) FROM urls").fetchone()
print(f"Gefunden (alle Zeilen) {total}, angezeigt {shown}, Differenz {total - shown}")
