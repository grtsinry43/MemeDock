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
# Homebrew and rustup both live outside the default PATH in a bare shell.
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
target=${MACOS_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
case "$target" in
  *-apple-darwin) ;;
  *) echo "Unsupported macOS target: $target" >&2; exit 1 ;;
esac
apps="$root/apps/macos"
generated="$apps/Generated"
work="$root/target/$target/$configuration"
version=$(python3 "$root/tools/release/version.py")

echo "==> Native media ($target)"
bash "$root/tools/native-media/build.sh" "$target"

echo "==> Rust cdylib ($configuration)"
cargo_args=(build --locked --target "$target" -p memedock-ffi -p memedock-bindgen)
if [[ "$configuration" == release ]]; then cargo_args+=(--release); fi
(cd "$root" && cargo "${cargo_args[@]}")

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
swift_args=(-c "$configuration" --package-path "$apps")
swift_args+=(-Xlinker "-L$work")
swift_args+=(-Xlinker -rpath -Xlinker "$work")
swift_args+=(-Xlinker -rpath -Xlinker "@executable_path/../Frameworks")
swift build "${swift_args[@]}"

echo "==> App bundle"
binary=$(swift build "${swift_args[@]}" --show-bin-path)/MemeDock
bundle="$apps/Bundle/MemeDock.app"
rm -rf "$bundle"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Frameworks" "$bundle/Contents/Resources"
cp "$binary" "$bundle/Contents/MacOS/MemeDock"
cp "$work/libmemedock_ffi.dylib" "$bundle/Contents/Frameworks/libmemedock_ffi.dylib"
sed "s/__VERSION__/$version/g" "$apps/Bundle/Info.plist" > "$bundle/Contents/Info.plist"
printf 'APPL????' > "$bundle/Contents/PkgInfo"
if command -v codesign >/dev/null 2>&1; then
  codesign --force --sign - "$bundle/Contents/Frameworks/libmemedock_ffi.dylib"
  codesign --force --sign - "$bundle"
fi
echo "Built $bundle"
