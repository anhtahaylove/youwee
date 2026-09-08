#!/usr/bin/env bash
# Ensures packaged Linux protocol handlers receive the URI from the desktop.
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 <deb-directory>" >&2
  exit 2
fi

DEB_DIR="$1"
found_deb=false

for deb_file in "$DEB_DIR"/*.deb; do
  [ -f "$deb_file" ] || continue
  found_deb=true

  package_dir="$(mktemp -d)"
  cleanup() {
    rm -rf "$package_dir"
  }
  trap cleanup RETURN

  dpkg-deb -R "$deb_file" "$package_dir"

  desktop_file="$package_dir/usr/share/applications/Youwee.desktop"
  test -f "$desktop_file"
  sed -i 's|^Exec=.*|Exec=youwee %U|' "$desktop_file"
  if grep -q '^MimeType=' "$desktop_file"; then
    sed -i 's|^MimeType=.*|MimeType=x-scheme-handler/youwee;|' "$desktop_file"
  else
    printf '\nMimeType=x-scheme-handler/youwee;\n' >> "$desktop_file"
  fi

  grep -Fx 'Exec=youwee %U' "$desktop_file"
  grep -Fqx 'MimeType=x-scheme-handler/youwee;' "$desktop_file"

  dpkg-deb --build "$package_dir" "$deb_file"
  trap - RETURN
  cleanup
done

if [ "$found_deb" = false ]; then
  echo "No .deb packages found in $DEB_DIR" >&2
  exit 1
fi
