#!/usr/bin/env bash
# Wrap the built .app in the installer disk image: background.svg rendered at 2x, a Finder
# window laid out by dmgbuild, and the Applications shortcut.
#
# Usage: make-dmg.sh <bundle.app> <output.dmg> [volume name]
#
# dmgbuild is the default, and needs no mount beyond the image it makes itself. Where that
# is refused -- a sandbox cannot write to /Volumes, and the failure is late and quiet: hdiutil
# reports success and the .dmg comes out empty -- set MEMEDOCK_DMG_BACKEND=nomount to lay the
# same volume out in a folder and hand it to hdiutil instead. See mkdmg.py.
#
# tools/macos/build.sh has already produced and signed the bundle, and
# tools/macos/make-icns.sh has already put its icon in place; this step only arranges them
# for a first run. The zip in the release workflow stays the canonical artifact, because a
# zip is what every other platform here ships too. The dmg is the alternative for people who
# expect one on macOS.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
bundle=${1:?Usage: $0 <bundle.app> <output.dmg> [volume name]}
output=${2:?Usage: $0 <bundle.app> <output.dmg> [volume name]}
volume=${3:-MemeDock Installer}
backend=${MEMEDOCK_DMG_BACKEND:-dmgbuild}

[[ -d "$bundle" ]] || { echo "Not a bundle: $bundle" >&2; exit 1; }
case "$output" in
  *.dmg) ;;
  *) echo "Output must end in .dmg: $output" >&2; exit 1 ;;
esac

work="$root/target/dmg"
mkdir -p "$work" "$(dirname "$output")"

# The background. The SVG asks for Kalam, a hand from Google Fonts; fetched here and
# registered for the render alone, so the machine need not have it installed and the runner
# does not need it either. Without the face the render still succeeds, only the line of help
# comes out in the fallback.
font="$work/Kalam-Regular.ttf"
if [[ ! -f $font ]]; then
  curl -fsSL -o "$font" \
    "https://raw.githubusercontent.com/google/fonts/main/ofl/kalam/Kalam-Regular.ttf" \
    || { rm -f "$font"; echo "note: Kalam unavailable, using the fallback face" >&2; }
fi
font_args=()
[[ -f $font ]] && font_args=("$font")
swift "$root/packaging/macos/render.swift" \
  "$root/packaging/macos/background.svg" "$work/background.png" 2 "${font_args[@]}"

# ds_store and mac_alias are needed by both backends: dmgbuild depends on them anyway, and
# mkdmg.py imports them directly.
venv="$work/venv"
if ! "$venv/bin/python3" -c 'import sys; sys.exit(sys.prefix == sys.base_prefix)' 2>/dev/null; then
  rm -rf "$venv"
  python3 -m venv "$venv"
fi
if ! "$venv/bin/python3" -c 'import dmgbuild, ds_store, mac_alias' 2>/dev/null; then
  "$venv/bin/python3" -m pip install --quiet dmgbuild ds_store mac_alias
fi

bundle_abs=$(cd "$(dirname "$bundle")" && pwd)/$(basename "$bundle")
case "$backend" in
dmgbuild)
  # dmgbuild attaches a scratch image and copies into it, so the bundle is named by its
  # absolute path and the definitions it receives have to be absolute too.
  "$venv/bin/python3" -m dmgbuild -s "$root/packaging/macos/installer.py" \
    -D "app=$bundle_abs" \
    -D "background=$work/background.png" \
    "$volume" "$output"
  echo "Built $output"
  ;;
nomount)
  "$venv/bin/python3" "$root/packaging/macos/mkdmg.py" \
    --app "$bundle_abs" \
    --background "$work/background.png" \
    --volume "$volume" \
    --out "$output"
  ;;
*)
  echo "Unknown MEMEDOCK_DMG_BACKEND: $backend (expected dmgbuild or nomount)" >&2
  exit 2
  ;;
esac