#!/usr/bin/env bash
# Neutralitätsprüfung (M2-H01, Nutzerentscheidung 16.10.2026): Das Referenzprodukt wird in versionierten Dateien
# nie mit Namen, Herstellernamen oder Kürzel genannt, sondern nur als „Referenzprodukt“ bzw. „Referenz“.
#
# Aufruf (aus jedem Ordner des Repositories):  scripts/check-neutral.sh
#
# Geprüft wird mit `git grep -n -I -P -i` über alle versionierten Textdateien. Gesucht werden:
#   1. das erste Wort des Herstellernamens (nicht die serde-Konstante in snake_case-Großschreibung),
#   2. der Produktname aus zwei Wörtern,
#   3. das Kürzel als ganzes Wort,
#   4. das Kürzel mit folgendem Unterstrich (Bezeichner),
#   5. das Kürzel mit anschließendem Bindestrich und dem Wort für die Gratis-Version.
# Die Namen stehen hier nur als Teilstrings, damit sich dieses Skript nicht selbst meldet. Ausgenommen sind außerdem
# per Pfad dieses Skript, seine Ausnahmedatei und die Messdaten tests/golden/parity/*/ref/*.csv.
#
# Ausnahmen gibt es nur über scripts/check-neutral.allow (Zeilen `Pfad<TAB>Teiltext<TAB>Grund`; `#` beginnt einen
# Kommentar). Eine Fundstelle ist erlaubt, wenn ein Eintrag mit demselben Pfad gilt und die Trefferzeile den Teiltext
# enthält. Einträge ohne Fundstelle sind verwaist und werden nur als Warnung ausgegeben.
#
# Ausgabe: je Fundstelle `Datei:Zeile`, dazu die gekürzte Zeile. Exit 0 ohne Fundstellen, 1 mit Fundstellen,
# 2 bei einem Fehler im Skript oder in der Ausnahmedatei.

set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"

allow_file="scripts/check-neutral.allow"
self="scripts/check-neutral.sh"

# Muster aus Teilstrings zusammensetzen.
m1='scre'; m2='aming'
p1='seo';  p2='spider'
k1='s';    k2='f'
maker="${m1}${m2}(?!_snake_case)"
product="${p1}\\s+${p2}"
abbr="${k1}${k2}"
pattern="${maker}|${product}|\\b${abbr}\\b|(?<![a-z])${abbr}_|\\b${abbr}-free"

# Ausnahmedatei einlesen (parallele Arrays; bash 3.2 kompatibel).
allow_paths=()
allow_texts=()
allow_used=()
allow_count=0
if [ -f "$allow_file" ]; then
  lineno=0
  while IFS= read -r raw || [ -n "$raw" ]; do
    lineno=$((lineno + 1))
    raw="${raw%$'\r'}"
    case "$raw" in
      '' | '#'*) continue ;;
    esac
    tab=$'\t'
    IFS="$tab" read -r a_path a_text a_reason <<< "$raw"
    if [ -z "${a_path:-}" ] || [ -z "${a_text:-}" ] || [ -z "${a_reason:-}" ]; then
      echo "$allow_file:$lineno: Eintrag braucht Pfad, Teiltext und Grund, getrennt durch Tabulatoren" >&2
      exit 2
    fi
    allow_paths+=("$a_path")
    allow_texts+=("$a_text")
    allow_used+=(0)
    allow_count=$((allow_count + 1))
  done < "$allow_file"
fi

# git grep liefert Exit 1 ohne Treffer; das ist hier der Gutfall.
hits="$(git grep -n -I -P -i -e "$pattern" -- . \
  ":(exclude)$self" \
  ":(exclude)$allow_file" \
  ':(exclude,glob)tests/golden/parity/*/ref/*.csv' || true)"

violations=0
if [ -n "$hits" ]; then
  while IFS= read -r hit; do
    [ -n "$hit" ] || continue
    h_path="${hit%%:*}"
    rest="${hit#*:}"
    h_line="${rest%%:*}"
    h_text="${rest#*:}"
    allowed=0
    i=0
    while [ "$i" -lt "$allow_count" ]; do
      if [ "${allow_paths[$i]}" = "$h_path" ] && [[ "$h_text" == *"${allow_texts[$i]}"* ]]; then
        allow_used[$i]=1
        allowed=1
        break
      fi
      i=$((i + 1))
    done
    if [ "$allowed" -eq 0 ]; then
      violations=$((violations + 1))
      if [ "${#h_text}" -gt 160 ]; then
        h_text="${h_text:0:160}..."
      fi
      echo "$h_path:$h_line: $h_text"
    fi
  done <<< "$hits"
fi

i=0
while [ "$i" -lt "$allow_count" ]; do
  if [ "${allow_used[$i]}" -eq 0 ]; then
    echo "Warnung: verwaiste Ausnahme in $allow_file (${allow_paths[$i]}): kein Treffer mehr, Eintrag entfernen" >&2
  fi
  i=$((i + 1))
done

if [ "$violations" -gt 0 ]; then
  echo "check-neutral: $violations Fundstelle(n) mit namentlichem Bezug auf das Referenzprodukt (nur „Referenzprodukt“ bzw. „Referenz“ verwenden)." >&2
  exit 1
fi
echo "check-neutral: keine Fundstellen."
