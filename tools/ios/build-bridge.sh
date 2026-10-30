#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd -P)
configuration=${1:-debug}
platform=${2:-${MEMEDOCK_IOS_PLATFORM:-all}}
case "$configuration" in
  debug|release) ;;
  *) echo "Usage: bash $0 [debug|release]" >&2; exit 1 ;;
esac
case "$platform" in
  all) targets=(aarch64-apple-ios aarch64-apple-ios-sim) ;;
  device) targets=(aarch64-apple-ios) ;;
  simulator) targets=(aarch64-apple-ios-sim) ;;
  *) echo "Usage: bash $0 [debug|release] [all|device|simulator]" >&2; exit 1 ;;
esac
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
cd "$root"
host=$(rustc -vV | sed -n 's/^host: //p')
if [[ "$host" != aarch64-apple-darwin ]]; then
  echo "This initial iOS build supports Apple silicon Macs (arm64 Simulator)." >&2
  exit 1
fi
generated="$root/apps/ios/Generated/$configuration"
artifacts_current() {
  local target slice
  [[ -f "$generated/MemeDockNative.xcframework/Info.plist" \
    && -f "$generated/Bridge/MemeDockBridge.swift" \
    && -f "$generated/FFI/module.modulemap" \
    && -f "$generated/FFI/MemeDockBridgeFFI.h" ]] || return 1
  for target in "${targets[@]}"; do
    slice=ios-arm64
    if [[ "$target" == aarch64-apple-ios-sim ]]; then slice=ios-arm64-simulator; fi
    [[ -f "$generated/MemeDockNative.xcframework/$slice/libmemedock_ffi.a" ]] || return 1
  done
}
mkdir -p "$generated"
lock="$root/apps/ios/Generated/build.lock"
while ! mkdir "$lock" 2>/dev/null; do
  owner=$(cat "$lock/pid" 2>/dev/null || true)
  if [[ -n "$owner" ]] && ! kill -0 "$owner" 2>/dev/null; then
    rm -rf "$lock"
  elif [[ -z "$owner" && -n $(find "$lock" -maxdepth 0 -mmin +2 2>/dev/null) ]]; then
    rm -rf "$lock"
  else
    sleep 1
  fi
done
printf '%s\n' "$$" > "$lock/pid"
trap 'rm -rf "$lock" "$generated/staging"' EXIT
input_stamp=$(
  {
    shasum -a 256 Cargo.toml Cargo.lock rust-toolchain.toml \
      "$root/tools/ios/build-bridge.sh" tools/native-media/build.sh tools/native-media/pkg-config.sh
    find crates/domain crates/storage crates/core crates/api-contracts crates/ffi \
      third-party/rlottie tools/uniffi-bindgen .cargo -type f \
      ! -path '*/target/*' ! -path '*/build/*' ! -path '*/.git/*' \
      -print | LC_ALL=C sort | while IFS= read -r path; do shasum -a 256 "$path"; done
    rustc -vV
    xcodebuild -version
    xcrun --sdk iphoneos --show-sdk-path
    xcrun --sdk iphonesimulator --show-sdk-path
    xcrun --sdk iphoneos --show-sdk-version
    xcrun --sdk iphonesimulator --show-sdk-version
    # Generated bridge artifacts must also follow Cargo profile/flag changes.
    env | LC_ALL=C sort | grep -E '^(CARGO_PROFILE_|CARGO_INCREMENTAL=|RUSTFLAGS=|CARGO_ENCODED_RUSTFLAGS=|RUSTC_WRAPPER=)' || true
    printf '%s\n' "$configuration" "$platform"
  } | shasum -a 256 | cut -d ' ' -f 1
)
if [[ -f "$generated/inputs" && $(cat "$generated/inputs") == "$input_stamp" ]] && artifacts_current; then
  echo "==> iOS bridge inputs are unchanged"
  exit 0
fi

for target in "${targets[@]}"; do
  if ! rustup target list --installed | grep -qx "$target"; then
    echo "Missing $target. Run: rustup target add ${targets[*]}" >&2
    exit 1
  fi
done
echo "==> Pinned UniFFI generator (host)"
env -u SDKROOT -u IPHONEOS_DEPLOYMENT_TARGET -u MACOSX_DEPLOYMENT_TARGET \
  -u CC -u CXX -u CFLAGS -u CXXFLAGS -u LDFLAGS \
  cargo build --locked --target "$host" -p memedock-bindgen
profile_args=()
if [[ "$configuration" == release ]]; then profile_args+=(--release); fi
for target in "${targets[@]}"; do
  sdk=iphoneos
  clang_target=arm64-apple-ios18.0
  if [[ "$target" == aarch64-apple-ios-sim ]]; then
    sdk=iphonesimulator
    clang_target+="-simulator"
  fi
  sdkroot=$(xcrun --sdk "$sdk" --show-sdk-path)
  echo "==> Native media ($target)"
  IPHONEOS_DEPLOYMENT_TARGET=18.0 bash "$root/tools/native-media/build.sh" "$target"
  echo "==> Rust static library ($target, $configuration)"
  host_sdk=$(xcrun --sdk macosx --show-sdk-path)
  env -u MACOSX_DEPLOYMENT_TARGET -u CC -u CXX -u CFLAGS -u CXXFLAGS -u LDFLAGS \
    SDKROOT="$sdkroot" IPHONEOS_DEPLOYMENT_TARGET=18.0 \
    "CC_$target=$(xcrun --sdk "$sdk" --find clang)" \
    "CXX_$target=$(xcrun --sdk "$sdk" --find clang++)" \
    "CFLAGS_$target=-target $clang_target -isysroot $sdkroot" \
    "CXXFLAGS_$target=-target $clang_target -isysroot $sdkroot" \
    HOST_CC="$(xcrun --sdk macosx --find clang)" \
    HOST_CFLAGS="-target arm64-apple-macos14.0 -isysroot $host_sdk" \
    BINDGEN_EXTRA_CLANG_ARGS="-target $clang_target -isysroot $sdkroot" \
    cargo rustc --locked -p memedock-ffi --lib --crate-type staticlib \
      --target "$target" ${profile_args[@]+"${profile_args[@]}"}
done

library="$root/target/${targets[0]}/$configuration/libmemedock_ffi.a"
stamp=$(
  {
    printf '%s\n' "$platform"
    shasum -a 256 "$root/tools/ios/build-bridge.sh" "$root/crates/ffi/uniffi.toml"
    for target in "${targets[@]}"; do
      shasum -a 256 "$root/target/$target/$configuration/libmemedock_ffi.a"
      media="$root/target/native-media/$target/install/lib"
      shasum -a 256 "$media/libavformat.a" "$media/libavfilter.a" "$media/libavcodec.a" \
        "$media/libswscale.a" "$media/libavutil.a" "$media/libvpx.a"
    done
  } | shasum -a 256 | cut -d ' ' -f 1
)
if [[ -f "$generated/complete" && $(cat "$generated/complete") == "$stamp" ]] && artifacts_current; then
  printf '%s\n' "$input_stamp" > "$generated/inputs"
  echo "==> XCFramework and Swift bindings are current"
  exit 0
fi
staging="$generated/staging"
mkdir -p "$staging/Bridge" "$staging/FFI"
echo "==> Generate Swift bindings from the iOS static archive"
"$root/target/$host/debug/memedock-bindgen" generate "$library" \
  --language swift --config "$root/crates/ffi/uniffi.toml" \
  --out-dir "$staging/Bridge" --no-format
mv "$staging/Bridge/MemeDockBridgeFFI.h" "$staging/FFI/"
mv "$staging/Bridge/MemeDockBridgeFFI.modulemap" "$staging/FFI/module.modulemap"
# ffmpeg-sys links these archives without Rust's static bundling modifier.
# Package them explicitly so the XCFramework has no external media paths.
framework_args=()
for target in "${targets[@]}"; do
  mkdir -p "$staging/$target"
  media="$root/target/native-media/$target/install/lib"
  xcrun libtool -static -D -no_warning_for_no_symbols \
    -o "$staging/$target/libmemedock_ffi.a" \
    "$root/target/$target/$configuration/libmemedock_ffi.a" \
    "$media/libavformat.a" "$media/libavfilter.a" "$media/libavcodec.a" \
    "$media/libswscale.a" "$media/libavutil.a" "$media/libvpx.a"
  framework_args+=(-library "$staging/$target/libmemedock_ffi.a" -headers "$staging/FFI")
done
echo "==> Package $platform slices"
xcodebuild -create-xcframework \
  "${framework_args[@]}" \
  -output "$staging/MemeDockNative.xcframework"
for name in Bridge FFI MemeDockNative.xcframework; do
  rm -rf "$generated/$name"
  mv "$staging/$name" "$generated/$name"
done
printf '%s\n' "$stamp" > "$generated/complete"
printf '%s\n' "$input_stamp" > "$generated/inputs"
echo "Built $generated/MemeDockNative.xcframework"
