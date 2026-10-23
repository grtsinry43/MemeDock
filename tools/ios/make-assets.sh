#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
assets="$root/apps/ios/Generated/Assets.xcassets"
icon="$assets/AppIcon.appiconset"
source_svg="$root/apps/linux/data/icons/hicolor/scalable/apps/com.grtsinry43.memedock.svg"
stamp=$(shasum -a 256 "$0" "$source_svg" | shasum -a 256 | cut -d ' ' -f 1)
if [[ -f "$assets/complete" && $(cat "$assets/complete") == "$stamp" && -f "$icon/AppIcon.png" ]]; then exit 0; fi
mkdir -p "$icon"
sips -z 1024 1024 -s format png "$source_svg" --out "$icon/AppIcon.png" >/dev/null
# iOS applies the icon's outer shape. Remove SVG transparency before compiling
# the asset; its sticker geometry remains the shared brand source.
sips -s format jpeg "$icon/AppIcon.png" --out "$icon/opaque.jpg" >/dev/null
sips -s format png "$icon/opaque.jpg" --out "$icon/AppIcon.png" >/dev/null
rm "$icon/opaque.jpg"
printf '%s\n' '{"info":{"author":"xcode","version":1}}' > "$assets/Contents.json"
printf '%s\n' '{"images":[{"filename":"AppIcon.png","idiom":"universal","platform":"ios","size":"1024x1024"}],"info":{"author":"xcode","version":1}}' > "$icon/Contents.json"
printf '%s\n' "$stamp" > "$assets/complete"
