#!/bin/sh
# M1-P32: Durchsatz je 10-%-Abschnitt waehrend eines `os-loadtest run` (Ersatz, bis N19 die Abschnitte in den Bericht schreibt).
#
# Liest alle <intervall> Sekunden `SELECT max(rowid) FROM links` der laufenden Crawl-Datei im Arbeitsordner
# (`sqlite3 -readonly`; WAL erlaubt das gleichzeitige Lesen), rechnet mit <links-je-seite> Links je Seite auf die
# gespeicherten Seiten um und bestimmt die Zeitpunkte, an denen 10 %, 20 % ... 100 % der Seiten gespeichert waren
# (lineare Interpolation zwischen zwei Stichproben). Daraus folgen Durchsatz je Abschnitt und Abweichung zum Mittel.
# Zaehlt die gespeicherten Seiten, nicht die abgerufenen: Der Writer schreibt gebuendelt, der Unterschied ist die
# Warteschlange des Writers (hoechstens Kanal + ein Gruppen-Commit).
#
# Aufruf:   scripts/bench/segments.sh <arbeitsordner> [seiten=250000] [links-je-seite=30] [intervall-s=2]
# Umgebung: OUT_JSON=<datei>  schreibt zusaetzlich einen JSON-Bericht.
#           CRAWL_FILE=<name> Dateiname der Crawl-Datei (Standard: main.ospider, slow.ospider oder crawl.ospider,
#                             je nachdem, welche zuerst im Arbeitsordner auftaucht; ref.ospider wird ignoriert).
# Beendet sich, sobald die Crawl-Datei nach ihrem Auftauchen wieder verschwindet (der Lasttest loescht sie am Ende)
# oder bei SIGINT/SIGTERM; gibt die Tabelle dann aus.
work="${1:?Arbeitsordner fehlt}"
pages="${2:-250000}"
lpp="${3:-30}"
every="${4:-2}"
samples=$(mktemp "${TMPDIR:-/tmp}/segments.XXXXXX") || exit 1
trap 'report; rm -f "$samples"; exit 0' INT TERM

now() { python3 -c 'import time; print("%.3f" % time.time())'; }

find_file() {
  if [ -n "$CRAWL_FILE" ]; then
    find "$work" -name "$CRAWL_FILE" -type f 2>/dev/null | head -n 1
  else
    find "$work" \( -name main.ospider -o -name slow.ospider -o -name crawl.ospider \) -type f 2>/dev/null | head -n 1
  fi
}

report() {
  [ -s "$samples" ] || { echo "segments.sh: keine Stichproben" >&2; return; }
  awk -v pages="$pages" -v lpp="$lpp" -v out="${OUT_JSON:-}" '
    { t[NR] = $1; p[NR] = $2 / lpp; if (p[NR] > pages) p[NR] = pages }
    END {
      n = NR; step = pages / 10
      # Zeitpunkt, zu dem k * step Seiten gespeichert waren
      t0 = t[1]; start = 0
      for (i = 1; i <= n; i++) if (p[i] > 0) { start = i; break }
      if (!start) { print "segments.sh: nichts gespeichert" > "/dev/stderr"; exit }
      prev = t[start > 1 ? start - 1 : start]; ts[0] = prev
      k = 1
      for (i = start; i <= n && k <= 10; i++) {
        while (k <= 10 && p[i] >= k * step - 1e-9) {
          if (i == 1) ts[k] = t[i]
          else {
            a = p[i - 1]; b = p[i]
            ts[k] = (b > a) ? t[i - 1] + (k * step - a) / (b - a) * (t[i] - t[i - 1]) : t[i]
          }
          k++
        }
      }
      done = k - 1
      if (done == 0) { print "segments.sh: kein Abschnitt vollstaendig" > "/dev/stderr"; exit }
      total = ts[done] - ts[0]
      mean = (done * step) / total
      printf "Seiten (Ziel): %d, Abschnitte vollstaendig: %d, Stichproben: %d\n", pages, done, n
      printf "%-9s %10s %12s %12s\n", "Abschnitt", "Dauer s", "Seiten/s", "zum Mittel"
      json = ""
      for (k = 1; k <= done; k++) {
        d = ts[k] - ts[k - 1]; r = step / d
        printf "%-9d %10.1f %12.0f %+11.0f%%\n", k, d, r, (r / mean - 1) * 100
        json = json (k > 1 ? "," : "") sprintf("{\"segment\":%d,\"seconds\":%.2f,\"pages_per_s\":%.1f,\"rel_to_mean\":%.4f}", k, d, r, r / mean)
        if (k == 1) { mn = r; mx = r } else { if (r < mn) mn = r; if (r > mx) mx = r }
      }
      printf "Mittel: %.0f Seiten/s ueber %.0f s; kleinster/groesster Abschnitt: %.0f%% / %.0f%% des Mittels\n", mean, total, mn / mean * 100, mx / mean * 100
      if (out != "")
        printf "{\"pages\":%d,\"links_per_page\":%d,\"segments_complete\":%d,\"mean_pages_per_s\":%.1f,\"min_rel\":%.4f,\"max_rel\":%.4f,\"segments\":[%s]}\n", pages, lpp, done, mean, mn / mean, mx / mean, json > out
    }' "$samples"
}

file=""
seen=0
while :; do
  if [ -z "$file" ]; then
    file=$(find_file)
  fi
  if [ -n "$file" ]; then
    if [ -f "$file" ]; then
      seen=1
      n=$(sqlite3 -readonly -cmd '.timeout 2000' "$file" 'SELECT COALESCE(max(rowid), 0) FROM links' 2>/dev/null)
      [ -n "$n" ] && echo "$(now) $n" >> "$samples"
    elif [ "$seen" = 1 ]; then
      break
    fi
  fi
  sleep "$every"
done
report
rm -f "$samples"
