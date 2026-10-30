#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
assets="$root/apps/ios/Generated/Assets.xcassets"
icon="$assets/AppIcon.appiconset"
source_svg="$root/apps/linux/data/icons/hicolor/scalable/apps/com.grtsinry43.memedock.svg"
stamp=$(shasum -a 256 "$0" "$source_svg" | shasum -a 256 | cut -d ' ' -f 1)
if [[ -f "$assets/complete" && $(cat "$assets/complete") == "$stamp" && -f "$icon/AppIcon.png" ]]; then exit 0; fi
mkdir -p "$icon"
# iOS applies the icon's outer shape, so the plate must fill the canvas: crop the
# view to the plate and square its corners. The shared SVG stays the brand source.
full_bleed="$icon/full-bleed.svg"
sed -e 's/viewBox="0 0 128 128"/viewBox="8 8 112 112"/' -e 's/ rx="26"//' "$source_svg" > "$full_bleed"
grep -q 'viewBox="8 8 112 112"' "$full_bleed" && ! grep -q 'rx="26"' "$full_bleed" || {
  echo "Unexpected icon source layout: $source_svg" >&2
  exit 1
}
sips -z 1024 1024 -s format png "$full_bleed" --out "$icon/AppIcon.png" >/dev/null
rm "$full_bleed"
# Remove any remaining transparency before compiling the asset.
sips -s format jpeg "$icon/AppIcon.png" --out "$icon/opaque.jpg" >/dev/null
sips -s format png "$icon/opaque.jpg" --out "$icon/AppIcon.png" >/dev/null
rm "$icon/opaque.jpg"
printf '%s\n' '{"info":{"author":"xcode","version":1}}' > "$assets/Contents.json"
printf '%s\n' '{"images":[{"filename":"AppIcon.png","idiom":"universal","platform":"ios","size":"1024x1024"}],"info":{"author":"xcode","version":1}}' > "$icon/Contents.json"
printf '%s\n' "$stamp" > "$assets/complete"
