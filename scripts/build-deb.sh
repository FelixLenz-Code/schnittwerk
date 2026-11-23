#!/usr/bin/env bash
# Builds the .deb with a fresh patch version, so `apt install ./…deb`
# really upgrades (apt skips packages whose version is already installed).
set -euo pipefail
cd "$(dirname "$0")/.."

current=$(grep -m1 '^version = ' Cargo.toml | sed -E 's/version = "(.*)"/\1/')
IFS=. read -r major minor patch <<<"$current"
next="$major.$minor.$((patch + 1))"
if [[ "${1:-}" == "--keep" ]]; then next="$current"; fi

sed -i -E "0,/^version = \".*\"/s//version = \"$next\"/" Cargo.toml
sed -i -E "s/\"version\": \"[0-9.]+\"/\"version\": \"$next\"/" src-tauri/tauri.conf.json package.json

source "$HOME/.cargo/env" 2>/dev/null || true
npm run tauri build
echo
echo "Fertig: target/release/bundle/deb/Schnittwerk_${next}_amd64.deb"
echo "Installieren: sudo apt install ./target/release/bundle/deb/Schnittwerk_${next}_amd64.deb"
