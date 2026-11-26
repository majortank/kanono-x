#!/usr/bin/env bash
set -euo pipefail

prefix="${PREFIX:-/usr/local}"
project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

cargo build --release --locked
install -Dm755 "$project_dir/target/release/kanono-x" "$prefix/bin/kanono-x"
install -Dm644 "$project_dir/resources/icons/kanono-x.svg" \
  "$prefix/share/icons/hicolor/scalable/apps/kanono-x.svg"
install -Dm644 "$project_dir/packaging/kanono-x.desktop" \
  "$prefix/share/applications/kanono-x.desktop"
install -Dm644 "$project_dir/packaging/kanono-x-autostart.desktop" \
  "$prefix/share/xdg/autostart/kanono-x.desktop"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$prefix/share/applications" || true
fi

echo "Installed Kanono X system-wide under $prefix"
echo "Kanono X will start hidden with tray controls on supported desktops."
