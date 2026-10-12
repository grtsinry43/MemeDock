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
(cd "$root" && cargo "${cargo_args[@]}")

echo "==> LINKEDIT string pool alignment"
# The linker starts the string pool right after the indirect symbol table, so
# an odd number of indirect symbols leaves the pool four bytes off a pointer
# boundary; `ld` and `dyld` then both reject the image. Re-emit the padding the
# linker left out before anything links against or loads the dylib.
"$python" "$root/tools/macos/align-string-pool.py" "$work/libmemedock_ffi.dylib"

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
# The executable is linked by the same toolchain, so give it the same string
# pool check before the bundle is signed.
"$python" "$root/tools/macos/align-string-pool.py" \
  "$bundle/Contents/MacOS/MemeDock" "$bundle/Contents/Frameworks/libmemedock_ffi.dylib"
if command -v codesign >/dev/null 2>&1; then
  codesign --force --sign - "$bundle/Contents/Frameworks/libmemedock_ffi.dylib"
  codesign --force --sign - "$bundle"
fi
echo "Built $bundle"
