#!/usr/bin/env bash
# Rückbau des Schattenbetriebs (M2-V02). Wird NUR ausgeliefert, nicht ausgeführt.
# Auf dem VPS als root starten (ssh -t root@... /opt/openspider-shadow/bin/uninstall.sh):
#   1. Timer und Dienst stoppen, Timer deaktivieren
#   2. Units entlinken (nur die Symlinks in /etc/systemd/system verschwinden), daemon-reload
#   3. Benutzer openspider-shadow und /opt/openspider-shadow nur nach ausdrücklicher Rückfrage entfernen
# Alle anderen Dienste, Units, Timer und Konfigurationen bleiben unberührt.
set -euo pipefail

BASE="/opt/openspider-shadow"
USER_NAME="openspider-shadow"

if [ "$(id -u)" != 0 ]; then echo "Bitte als root ausführen." >&2; exit 1; fi

ask() { # ask "Frage" -> 0 bei Ja
  local answer
  read -r -p "$1 [ja/N] " answer
  [ "$answer" = "ja" ]
}

echo "Stoppe Timer und Dienst ..."
systemctl disable --now openspider-shadow.timer 2>/dev/null || true
systemctl stop openspider-shadow.service 2>/dev/null || true

echo "Entlinke Units ..."
for unit in openspider-shadow.timer openspider-shadow.service; do
  if [ -L "/etc/systemd/system/$unit" ]; then
    systemctl disable "$unit" 2>/dev/null || true
    rm -f "/etc/systemd/system/$unit"
  fi
done
systemctl daemon-reload
systemctl reset-failed 'openspider-shadow.*' 2>/dev/null || true

if [ -d "$BASE" ] && ask "Verzeichnis $BASE samt Crawls, Exporten und Berichten (var/) löschen?"; then
  rm -rf -- "$BASE"
  echo "$BASE entfernt."
else
  echo "$BASE bleibt bestehen."
fi

if id "$USER_NAME" >/dev/null 2>&1 && ask "Systembenutzer $USER_NAME löschen?"; then
  userdel "$USER_NAME"
  echo "Benutzer $USER_NAME entfernt."
else
  echo "Benutzer $USER_NAME bleibt bestehen."
fi
echo "Fertig. systemctl list-unit-files 'openspider-shadow*' sollte nichts mehr zeigen."
