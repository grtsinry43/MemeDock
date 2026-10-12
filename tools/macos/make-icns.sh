#!/usr/bin/env bash
# Build the .icns resource that the macOS bundle declares through
# CFBundleIconFile. The shared brand mark stays the single source of truth, so
# macOS cannot drift from the Linux SVG and the Android adaptive icon that draw
# the same geometry.
#
# sips reads SVG through ImageIO and rasterises the vectors at the size it is
# asked for, so every iconset entry is drawn at its final resolution instead of
# being resampled from one bitmap. Quick Look cannot be used here: its SVG
# thumbnails come back with an opaque white page background.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
usage() { echo "Usage: $0 <output.icns> [source.svg]" >&2; exit 1; }
[[ $# -ge 1 && $# -le 2 ]] || usage
output=$1
source_svg=${2:-"$root/apps/linux/data/icons/hicolor/scalable/apps/com.grtsinry43.memedock.svg"}
case "$output" in
  *.icns) ;;
  *) echo "Output must end in .icns: $output" >&2; exit 1 ;;
esac
if [[ ! -f "$source_svg" ]]; then
  echo "Missing icon source: $source_svg" >&2
  exit 1
fi
work=$(mktemp -d "${TMPDIR:-/tmp}/memedock-icon.XXXXXX")
trap 'rm -rf "$work"' EXIT
iconset="$work/AppIcon.iconset"
mkdir -p "$iconset"
# iconutil rejects an iconset whose names or pixel sizes do not match exactly.
for points in 16 32 128 256 512; do
  for scale in 1 2; do
    pixels=$((points * scale))
    name="icon_${points}x${points}"
    if [[ $scale -eq 2 ]]; then name="${name}@2x"; fi
    entry="$iconset/$name.png"
    if ! sips -z "$pixels" "$pixels" -s format png "$source_svg" --out "$entry" >/dev/null; then
      echo "sips could not rasterise $source_svg as $pixels px (ImageIO needs SVG support)." >&2
      exit 1
    fi
    actual=$(sips -g pixelWidth "$entry" | awk '/pixelWidth/ {print $2}')
    if [[ "$actual" != "$pixels" ]]; then
      echo "Expected $name.png to measure $pixels px, got $actual" >&2
      exit 1
    fi
  done
done
mkdir -p "$(dirname "$output")"
iconutil -c icns "$iconset" -o "$output"
