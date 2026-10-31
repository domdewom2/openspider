#!/usr/bin/env bash
# Only called by the published-release workflow after explicit M3-REL opt-in.
set -euo pipefail
: "${RELEASE_TAG:?}" "${OPENSPIDER_PACKAGE_ID:?}" "${HOMEBREW_TAP_REPOSITORY:?}" "${WINGET_FORK_REPOSITORY:?}"
: "${HOMEBREW_TAP_TOKEN:?}" "${WINGET_PR_TOKEN:?}"
[[ "$RELEASE_TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]
[[ "$OPENSPIDER_PACKAGE_ID" =~ ^[A-Za-z0-9]+(\.[A-Za-z0-9]+)+$ ]]
[[ "$HOMEBREW_TAP_REPOSITORY" =~ ^[A-Za-z0-9_.-]+/homebrew-[A-Za-z0-9_.-]+$ ]]
[[ "$WINGET_FORK_REPOSITORY" =~ ^[A-Za-z0-9_.-]+/winget-pkgs$ ]]
[[ "$WINGET_FORK_REPOSITORY" != microsoft/winget-pkgs ]]
metadata="$PWD/generated"
test -s "$metadata/openspider.rb"
test -s "$metadata/$OPENSPIDER_PACKAGE_ID.installer.yaml"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
export GH_TOKEN="$HOMEBREW_TAP_TOKEN"
# gh (MIT) uses the token from the environment. No token in a URL, argument or log.
gh repo clone "$HOMEBREW_TAP_REPOSITORY" "$scratch/tap" -- --depth=1
git -C "$scratch/tap" config user.name 'OpenSpider release'
git -C "$scratch/tap" config user.email 'release@openspider.de'
mkdir -p "$scratch/tap/Casks"
cp "$metadata/openspider.rb" "$scratch/tap/Casks/openspider.rb"
git -C "$scratch/tap" add Casks/openspider.rb
if ! git -C "$scratch/tap" diff --cached --quiet; then
  git -C "$scratch/tap" commit -m "openspider ${RELEASE_TAG#v}"
  git -C "$scratch/tap" -c credential.helper= -c 'credential.helper=!gh auth git-credential' push origin HEAD
fi
export GH_TOKEN="$WINGET_PR_TOKEN"
gh repo clone "$WINGET_FORK_REPOSITORY" "$scratch/winget" -- --depth=1
cd "$scratch/winget"
git config user.name 'OpenSpider release'
git config user.email 'release@openspider.de'
# Derive the directory from the approved identifier, not the proposed project name.
path="manifests/${OPENSPIDER_PACKAGE_ID:0:1}"
path="${path,,}/${OPENSPIDER_PACKAGE_ID//./\/}/${RELEASE_TAG#v}"
branch="openspider-${RELEASE_TAG#v}"
git switch -c "$branch"
mkdir -p "$path"
cp "$metadata/"*.yaml "$path/"
git add "$path"
git commit -m "New version: $OPENSPIDER_PACKAGE_ID ${RELEASE_TAG#v}"
git -c credential.helper= -c 'credential.helper=!gh auth git-credential' push origin "$branch"
printf 'Generated from the published OpenSpider release %s. MSI: machine; NSIS: user.\n' "$RELEASE_TAG" > "$scratch/body.md"
gh pr create --repo microsoft/winget-pkgs --head "${WINGET_FORK_REPOSITORY%%/*}:$branch" --base master \
  --title "New version: $OPENSPIDER_PACKAGE_ID ${RELEASE_TAG#v}" --body-file "$scratch/body.md"
