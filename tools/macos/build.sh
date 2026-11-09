#!/usr/bin/env bash
# Build the macOS application: pinned native media, the UniFFI cdylib, the
# generated Swift bindings, and the .app bundle. Generated files stay in
# apps/macos/Generated and apps/macos/.build, which the package ignores.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
configuration=${1:-debug}
case "$configuration" in
  debug|release) ;;
  *) echo "Usage: $0 [debug|release]" >&2; exit 1 ;;
esac
# Homebrew, rustup and uv all live outside the default PATH in a bare shell.
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
target=${MACOS_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
case "$target" in
  *-apple-darwin) ;;
  *) echo "Unsupported macOS target: $target" >&2; exit 1 ;;
esac
apps="$root/apps/macos"
generated="$apps/Generated"
work="$root/target/$target/$configuration"
# tools/release/version.py reads TOML through tomllib, which requires Python
# 3.11; the interpreter bundled with macOS is older, so prefer a newer one
# instead of failing with an import error.
python=
for candidate in python3.13 python3.12 python3.11 python3 python; do
  if command -v "$candidate" >/dev/null 2>&1 && "$candidate" -c 'import tomllib' >/dev/null 2>&1; then
    python=$candidate
    break
  fi
done
if [[ -z $python ]]; then
  echo "Need Python >= 3.11 for tools/release/version.py (tomllib)." >&2
  echo "Install one, for example: uv python install 3.12" >&2
  exit 1
fi
version=$("$python" "$root/tools/release/version.py")

echo "==> Native media ($target)"
bash "$root/tools/native-media/build.sh" "$target"

echo "==> Rust cdylib ($configuration)"
cargo_args=(build --locked --target "$target" -p memedock-ffi -p memedock-bindgen)
if [[ "$configuration" == release ]]; then cargo_args+=(--release); fi
# Avoid the Mach-O stripping bug tracked in rust-lang/rust#157750.
(cd "$root" && CARGO_PROFILE_DEV_STRIP=none CARGO_PROFILE_RELEASE_STRIP=none \
  cargo "${cargo_args[@]}")

echo "==> UniFFI Swift bindings"
rm -rf "$generated/FFI" "$generated/Bridge"
mkdir -p "$generated/FFI" "$generated/Bridge"
"$work/memedock-bindgen" generate "$work/libmemedock_ffi.dylib" \
  --language swift \
  --config "$root/crates/ffi/uniffi.toml" \
  --out-dir "$generated/Bridge" \
  --no-format
# SwiftPM's system-library target requires the module map to be named
# `module.modulemap`; the generated header keeps its module name.
mv "$generated/Bridge/MemeDockBridgeFFI.modulemap" "$generated/FFI/module.modulemap"
mv "$generated/Bridge/MemeDockBridgeFFI.h" "$generated/FFI/MemeDockBridgeFFI.h"

echo "==> Swift build"
# Keep the pinned dependency's localization lookup compatible with a signed
# macOS app. SwiftPM's native Bundle.module accessor misses Contents/Resources.
swift package --package-path "$apps" resolve
shortcuts="$apps/.build/checkouts/KeyboardShortcuts"
resource_patch="$root/tools/macos/keyboardshortcuts-resources.patch"
if [[ $(git -C "$shortcuts" rev-parse HEAD) != 772133d9dbe800fdac0473226822994c5c162c58 ]]; then
  echo "KeyboardShortcuts resource patch requires the pinned 3.1.0 revision." >&2
  exit 1
fi
if git -C "$shortcuts" apply --check "$resource_patch" 2>/dev/null; then
  git -C "$shortcuts" apply "$resource_patch"
elif ! git -C "$shortcuts" apply --reverse --check "$resource_patch"; then
  echo "KeyboardShortcuts resource patch does not match the checkout." >&2
  exit 1
fi
swift_args=(-c "$configuration" --package-path "$apps")
swift_args+=(-Xlinker "-L$work")
swift_args+=(-Xlinker -rpath -Xlinker "$work")
swift_args+=(-Xlinker -rpath -Xlinker "@executable_path/../Frameworks")
swift build "${swift_args[@]}"

echo "==> App bundle"
swift_products=$(swift build "${swift_args[@]}" --show-bin-path)
binary="$swift_products/MemeDock"
bundle="$apps/Bundle/MemeDock.app"
rm -rf "$bundle"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Frameworks" "$bundle/Contents/Resources"
echo "==> App icon"
# Info.plist names AppIcon, and the artwork comes from the shared brand mark
# rather than a macOS-only copy of it.
bash "$root/tools/macos/make-icns.sh" "$bundle/Contents/Resources/AppIcon.icns"
cp "$binary" "$bundle/Contents/MacOS/MemeDock"
# The localization patch above loads this bundle from Contents/Resources in
# packaged apps, retaining SwiftPM's normal lookup for development executables.
cp -R "$swift_products/KeyboardShortcuts_KeyboardShortcuts.bundle" "$bundle/Contents/Resources/"
cp "$apps/.build/checkouts/KeyboardShortcuts/license" "$bundle/Contents/Resources/KeyboardShortcuts-LICENSE.txt"
cp "$work/libmemedock_ffi.dylib" "$bundle/Contents/Frameworks/libmemedock_ffi.dylib"
# Reuse the existing variable Latin font and its license. Chinese text cascades
# to macOS PingFang rather than bundling the Android Chinese font.
cp "$root/apps/android/app/src/main/res/font/memedock_quicksand.ttf" \
  "$bundle/Contents/Resources/MemeDockQuicksand.ttf"
cp "$root/apps/android/app/src/main/assets/font_licenses/quicksand-OFL.txt" \
  "$bundle/Contents/Resources/Quicksand-OFL.txt"
# Cargo records the absolute build path as the dylib's install name, so the
# copied executable would keep loading from target/ and ignore the bundled
# copy. Repoint both sides at the rpath the linker settings already provide.
linked=$(otool -L "$bundle/Contents/MacOS/MemeDock" | awk '/libmemedock_ffi\.dylib/ {print $1; exit}')
install_name_tool -id @rpath/libmemedock_ffi.dylib "$bundle/Contents/Frameworks/libmemedock_ffi.dylib"
install_name_tool -change "$linked" @rpath/libmemedock_ffi.dylib "$bundle/Contents/MacOS/MemeDock"
# The build-tree rpath is listed ahead of the bundle's own Frameworks, so drop
# it and leave `@executable_path/../Frameworks` as the only place to look.
if otool -l "$bundle/Contents/MacOS/MemeDock" | grep -qF "path $work (offset"; then
  install_name_tool -delete_rpath "$work" "$bundle/Contents/MacOS/MemeDock"
fi
sed "s/__VERSION__/$version/g" "$apps/Bundle/Info.plist" > "$bundle/Contents/Info.plist"
printf 'APPL????' > "$bundle/Contents/PkgInfo"
if command -v codesign >/dev/null 2>&1; then
  codesign --force --sign - "$bundle/Contents/Frameworks/libmemedock_ffi.dylib"
  codesign --force --sign - "$bundle"
fi
echo "Built $bundle"
