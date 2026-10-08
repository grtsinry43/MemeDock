#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
directory="$root/target/native-media/test-fixtures"
mkdir -p "$directory"
fetch() {
  local name=$1 hash=$2 url=$3
  if [[ ! -f "$directory/$name" ]]; then
    curl --fail --location --connect-timeout 15 --max-time 120 "$url" -o "$directory/$name.partial"
    mv "$directory/$name.partial" "$directory/$name"
  fi
  if [[ $(sha256sum "$directory/$name" | cut -d ' ' -f 1) != "$hash" ]]; then echo "Fixture checksum mismatch: $name" >&2; exit 1; fi
}
fetch telegram.tgs 0a433a3c4cfb59d7a5b78f49065c957cb9b045d7aa7e0cf401d54e9846c78a08 https://raw.githubusercontent.com/telegramdesktop/tdesktop/f23c37857220eb84f8559f0901ea26fb304b564b/Telegram/Resources/animations/chat/sparkles_emoji.tgs
fetch alpha.webm 4db46bc6c600c8a978cb88e2386af641ab1dd9b4e007242f41fd162530c81e80 https://raw.githubusercontent.com/laggykiller/sticker-convert/9050513b059e1545b9746df67a59b666f994cc85/tests/samples/animated_webm_320x240_2s_vp9a.webm
