#!/usr/bin/env bash
# Umgebung für Claude-Code-Cloud-Sessions und den Linux-Job `cloud-setup` (docs/plan/M2.md 2a C3, M2-F01).
#
# Richtet ein: Rust laut rust-toolchain.toml, Node 22 (engines), `npm ci`, Playwright (Chromium + WebKit mit
# Systemabhängigkeiten) sobald @playwright/test in package.json steht (M2-F04), OPENSPIDER_CHROMIUM, optional die
# webkit2gtk-Bibliotheken für `openspider-desktop` unter Linux und OPENSPIDER_SECRET_STORE=memory (C13: kein Secret
# Service, kein gnome-keyring). Idempotent: ein zweiter Lauf ändert nichts, was schon stimmt.
#
# Aufruf:  bash scripts/cloud/setup.sh [--check]
#   --check   prüft nur, schreibt und installiert nichts; Exit-Code 1, wenn etwas fehlt.
# Umgebung:
#   OPENSPIDER_SETUP_DESKTOP=1   zusätzlich libwebkit2gtk-4.1-dev librsvg2-dev libayatana-appindicator3-dev installieren
#   OPENSPIDER_SETUP_SKIP=a,b    Schritte überspringen: rust, node, npm, playwright, desktop, fetch
#   NODE_MAJOR                   Node-Hauptversion (Standard 22)
# Ausgabe der Umgebungsvariablen: $HOME/.openspider-cloud-env (zum Einlesen mit `source`), zusätzlich $GITHUB_ENV und
# $CLAUDE_ENV_FILE, falls gesetzt.

set -euo pipefail

CHECK=0
for arg in "$@"; do
  case "$arg" in
    --check) CHECK=1 ;;
    -h | --help) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unbekanntes Argument: $arg" >&2; exit 2 ;;
  esac
done

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

NODE_MAJOR="${NODE_MAJOR:-22}"
SKIP=",${OPENSPIDER_SETUP_SKIP:-},"
ENV_FILE="${HOME}/.openspider-cloud-env"
MISSING=0

log() { printf '[cloud-setup] %s\n' "$*"; }
skipped() { [[ "$SKIP" == *",$1,"* ]]; }
have() { command -v "$1" >/dev/null 2>&1; }
as_root() { if [[ "$(id -u)" -eq 0 ]]; then "$@"; elif have sudo; then sudo "$@"; else echo "root-Rechte fehlen für: $*" >&2; return 1; fi; }
missing() { log "FEHLT: $*"; MISSING=1; }

# --- Rust ----------------------------------------------------------------------------------------------------
step_rust() {
  if ! have rustup; then
    if [[ $CHECK -eq 1 ]]; then missing "rustup"; return; fi
    log "installiere rustup (ohne Standard-Toolchain)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
  fi
  # shellcheck disable=SC1091
  [[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env"
  if [[ $CHECK -eq 1 ]]; then
    have cargo && cargo --version >/dev/null 2>&1 || missing "Rust-Toolchain laut rust-toolchain.toml"
    return
  fi
  log "installiere die Toolchain aus rust-toolchain.toml"
  rustup toolchain install # liest rust-toolchain.toml (Kanal, Komponenten, Profil)
  cargo --version
}

# --- Node ----------------------------------------------------------------------------------------------------
node_ok() { have node && [[ "$(node --version | sed 's/^v//; s/\..*//')" -ge "$NODE_MAJOR" ]]; }

step_node() {
  if node_ok; then log "Node $(node --version) vorhanden"; return; fi
  if [[ $CHECK -eq 1 ]]; then missing "Node >= $NODE_MAJOR"; return; fi
  local os arch dist base file ver prefix="$HOME/.local/node"
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  case "$(uname -m)" in x86_64 | amd64) arch=x64 ;; aarch64 | arm64) arch=arm64 ;; *) echo "unbekannte Architektur $(uname -m)" >&2; return 1 ;; esac
  base="https://nodejs.org/dist/latest-v${NODE_MAJOR}.x"
  file="$(curl -fsSL "$base/SHASUMS256.txt" | awk -v p="node-v${NODE_MAJOR}[.0-9]*-${os}-${arch}.tar.xz" '$2 ~ p {print $2; exit}')"
  [[ -n "$file" ]] || { echo "keine Node-${NODE_MAJOR}-Datei für ${os}-${arch} gefunden" >&2; return 1; }
  ver="${file%.tar.xz}"
  dist="$(mktemp -d)"
  log "lade $file"
  curl -fsSL "$base/$file" -o "$dist/$file"
  (cd "$dist" && curl -fsSL "$base/SHASUMS256.txt" | grep " $file\$" | (sha256sum -c - 2>/dev/null || shasum -a 256 -c -))
  mkdir -p "$prefix"
  rm -rf "${prefix:?}/$ver"
  tar -xJf "$dist/$file" -C "$prefix"
  ln -sfn "$prefix/$ver" "$prefix/current"
  export PATH="$prefix/current/bin:$PATH"
  EXTRA_PATH="$prefix/current/bin"
  rm -rf "$dist"
  log "Node $(node --version) nach $prefix"
}

# --- npm -----------------------------------------------------------------------------------------------------
step_npm() {
  if [[ $CHECK -eq 1 ]]; then
    [[ -d node_modules ]] || missing "node_modules (npm ci)"
    return
  fi
  log "npm ci"
  npm ci --no-audit --no-fund
}

# --- Playwright ----------------------------------------------------------------------------------------------
playwright_present() { [[ -d node_modules/@playwright/test ]] || [[ -d node_modules/playwright ]]; }

step_playwright() {
  if ! playwright_present; then
    log "Playwright steht noch nicht in package.json (M2-F04): übersprungen"
    return
  fi
  if [[ $CHECK -eq 1 ]]; then
    [[ -n "$(chromium_path)" && -e "$(chromium_path)" ]] || missing "Playwright-Chromium"
    return
  fi
  local deps=()
  [[ "$(uname -s)" == "Linux" ]] && deps=(--with-deps)
  log "installiere Playwright-Browser (chromium, webkit)"
  npx --no-install playwright install "${deps[@]}" chromium webkit
}

chromium_path() {
  playwright_present || return 0
  node -e "try{console.log(require('playwright').chromium.executablePath())}catch(e){try{console.log(require('@playwright/test').chromium.executablePath())}catch(e2){}}" 2>/dev/null || true
}

# --- webkit2gtk für openspider-desktop unter Linux ---------------------------------------------------------------
step_desktop() {
  [[ "$(uname -s)" == "Linux" && "${OPENSPIDER_SETUP_DESKTOP:-0}" == "1" ]] || { log "Desktop-Bibliotheken: übersprungen (OPENSPIDER_SETUP_DESKTOP=1 und Linux nötig)"; return; }
  if [[ $CHECK -eq 1 ]]; then dpkg -s libwebkit2gtk-4.1-dev >/dev/null 2>&1 || missing "libwebkit2gtk-4.1-dev"; return; fi
  log "installiere webkit2gtk-Bibliotheken"
  as_root apt-get update -y
  as_root apt-get install -y --no-install-recommends libwebkit2gtk-4.1-dev librsvg2-dev libayatana-appindicator3-dev
}

# --- Abhängigkeiten vorladen -------------------------------------------------------------------------------------
step_fetch() {
  [[ $CHECK -eq 1 ]] && return
  have cargo || return 0
  log "cargo fetch --locked"
  cargo fetch --locked
}

# --- Umgebung ------------------------------------------------------------------------------------------------
write_env() {
  local chromium=""
  chromium="$(chromium_path)"
  local lines=("OPENSPIDER_SECRET_STORE=memory")
  [[ -n "$chromium" ]] && lines+=("OPENSPIDER_CHROMIUM=$chromium")
  if [[ $CHECK -eq 1 ]]; then
    [[ "${OPENSPIDER_SECRET_STORE:-}" == "memory" ]] || log "Hinweis: OPENSPIDER_SECRET_STORE ist nicht gesetzt (source $ENV_FILE)"
    return
  fi
  {
    for l in "${lines[@]}"; do printf 'export %s\n' "$l"; done
    [[ -n "${EXTRA_PATH:-}" ]] && printf 'export PATH="%s:$PATH"\n' "$EXTRA_PATH"
    # shellcheck disable=SC2016
    printf '[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"\n'
  } > "$ENV_FILE"
  log "Umgebung nach $ENV_FILE geschrieben"
  for target in "${GITHUB_ENV:-}" "${CLAUDE_ENV_FILE:-}"; do
    [[ -n "$target" ]] || continue
    if [[ "$target" == "${GITHUB_ENV:-}" ]]; then
      printf '%s\n' "${lines[@]}" >> "$target"
    else
      for l in "${lines[@]}"; do printf 'export %s\n' "$l" >> "$target"; done
    fi
  done
  if [[ -n "${GITHUB_PATH:-}" && -n "${EXTRA_PATH:-}" ]]; then echo "$EXTRA_PATH" >> "$GITHUB_PATH"; fi
}

EXTRA_PATH=""
for step in rust node npm playwright desktop fetch; do
  if skipped "$step"; then log "Schritt $step übersprungen"; continue; fi
  "step_$step"
done
write_env

if [[ $CHECK -eq 1 ]]; then
  [[ $MISSING -eq 0 ]] && log "alles vorhanden" || { log "unvollständig"; exit 1; }
else
  log "fertig"
fi
