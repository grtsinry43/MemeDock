#!/usr/bin/env bash
# Pinned, decoder-only media dependencies. All generated files stay in target/.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
mode=build
if [[ ${1:-} == --verify ]]; then mode=verify; shift; fi
target=${1:?Rust target required}
darwin=0
pkg_config=/usr/bin/pkg-config
case "$target" in
  x86_64-unknown-linux-gnu) arch=x86_64; vpx_target=x86_64-linux-gcc; abi= ;;
  aarch64-linux-android) arch=aarch64; vpx_target=arm64-linux-gcc; abi=arm64-v8a ;;
  x86_64-linux-android) arch=x86_64; vpx_target=x86_64-linux-gcc; abi=x86_64 ;;
  # The versioned Darwin targets denote macOS (darwin23 == macOS 14); the
  # versionless "arm64-darwin-gcc" target builds for iOS instead.
  aarch64-apple-darwin) arch=arm64; vpx_target=arm64-darwin23-gcc; abi=; darwin=1 ;;
  x86_64-apple-darwin) arch=x86_64; vpx_target=x86_64-darwin23-gcc; abi=; darwin=1 ;;
  *) echo "Unsupported native media target: $target" >&2; exit 1 ;;
esac
if (( darwin )); then
  # Darwin ships no /usr/bin/pkg-config; the Homebrew one answers the queries.
  pkg_config=$(command -v pkg-config || true)
  for candidate in /opt/homebrew/bin/pkg-config /usr/local/bin/pkg-config; do
    if [[ -x "$candidate" ]]; then pkg_config="$candidate"; break; fi
  done
  if [[ ! -x "$pkg_config" ]]; then
    echo "Install pkg-config (Homebrew) to build the macOS native media stack" >&2; exit 1
  fi
  # Keep the static archives aligned with the oldest macOS version the app targets.
  export MACOSX_DEPLOYMENT_TARGET=${MACOSX_DEPLOYMENT_TARGET:-14.0}
fi
# flock(1) is Linux-only; Darwin uses an equivalent mkdir-based mutex. The mkdir
# mutex records its holder, so a killed build is reclaimed immediately instead of
# blocking every other pkg-config query until the lock merely looks old.
lock_acquire() { # $1 lock path, $2 file descriptor
  if command -v flock >/dev/null 2>&1; then
    eval "exec $2>\"$1\""
    flock "$2"
    return
  fi
  while ! mkdir "$1.lock" 2>/dev/null; do
    owner=$(cat "$1.lock/pid" 2>/dev/null || true)
    stale=0
    if [[ -z "$owner" ]]; then
      # The holder died between mkdir and recording its pid.
      if [[ -n "$(find "$1.lock" -maxdepth 0 -mmin +2 2>/dev/null)" ]]; then stale=1; fi
    elif ! kill -0 "$owner" 2>/dev/null && ! ps -p "$owner" >/dev/null 2>&1; then
      stale=1
    elif [[ -n "$(find "$1.lock" -maxdepth 0 -mmin +60 2>/dev/null)" ]]; then
      # A recycled pid keeps the holder looking alive; do not wait forever.
      stale=1
    fi
    if (( stale )); then rm -rf "$1.lock"; continue; fi
    sleep 1
  done
  printf '%s\n' "$$" > "$1.lock/pid"
}
lock_release() { # $1 lock path, $2 file descriptor
  if command -v flock >/dev/null 2>&1; then
    flock -u "$2" 2>/dev/null || true
  else
    rm -rf "$1.lock"
  fi
}
# Darwin ships shasum rather than sha256sum; both print "<hash>  <file>".
if ! command -v sha256sum >/dev/null 2>&1; then
  sha256sum() { shasum -a 256 "$@"; }
fi
base="$root/target/native-media"
work="$base/$target"
prefix="$work/install"
stamp=$(sha256sum "$0" | cut -d ' ' -f 1)
fresh() { [[ -f "$work/complete" && $(cat "$work/complete") == "$stamp" && -f "$prefix/lib/libavcodec.a" ]]; }
# --verify reports whether the cached build is current and takes no lock, so the
# pkg-config wrapper skips the mutex on the common "nothing changed" path.
if [[ "$mode" == verify ]]; then
  if fresh; then exit 0; else exit 1; fi
fi
mkdir -p "$work" "$base/sources"
lock_acquire "$work/build.lock" 9
# Never leave the lock behind: covers the fast path below, failures, and ^C.
trap 'lock_release "$work/build.lock" 9' EXIT
if fresh; then exit 0; fi
fetch() {
  local name=$1 url=$2 hash=$3 archive="$base/sources/$1.archive"
  if [[ ! -f "$archive" ]]; then
    curl --fail --location --connect-timeout 15 --max-time 300 --retry 2 "$url" -o "$archive.partial"
    mv "$archive.partial" "$archive"
  fi
  if [[ $(sha256sum "$archive" | cut -d ' ' -f 1) != "$hash" ]]; then
    echo "Native source checksum mismatch: $name" >&2; exit 1
  fi
  if [[ ! -d "$base/sources/$name" ]]; then
    mkdir -p "$base/sources/$name.partial"
    tar -xf "$archive" --strip-components=1 -C "$base/sources/$name.partial"
    mv "$base/sources/$name.partial" "$base/sources/$name"
  fi
}
# Serialize source extraction across ABIs and concurrent Cargo build scripts.
lock_acquire "$base/sources.lock" 8
fetch rlottie https://codeload.github.com/msrd0/rlottie-telegram/tar.gz/f435c0a4f364ce39f7e4eee9ddc7118573da9bf9 7cfcd8fad8d245d58b41631c7f9b45042a8dc80d3221541293612723f0b84f79
fetch webp https://storage.googleapis.com/downloads.webmproject.org/releases/webp/libwebp-1.6.0.tar.gz e4ab7009bf0629fd11982d4c2aa83964cf244cffba7347ecd39019a9e38c4564
fetch vpx https://codeload.github.com/webmproject/libvpx/tar.gz/refs/tags/v1.17.0 1020f184046187baa2985dbde38e0691f49c44088bca7a1842b0236c6081dc0a
fetch ffmpeg https://ffmpeg.org/releases/ffmpeg-9.0.1.tar.xz cf38e0e28c7e5605942c4a77755349b0145804a397af37eb1fb4c77cb237f635
lock_release "$base/sources.lock" 8
cmake_args=()
ffmpeg_args=()
cpp_libs=-lstdc++
rlottie_args=()
if (( darwin )); then
  # Darwin links the system libc++ and needs no cross-compilation plumbing.
  cpp_libs=-lc++
  ffmpeg_args=(--target-os=darwin --arch="$arch")
  # rlottie ships 32-bit ARM NEON assembly only, while Apple clang defines
  # __ARM_NEON__ on arm64; undefine it so the portable C paths are used.
  rlottie_args=(-DCMAKE_C_FLAGS=-U__ARM_NEON__ -DCMAKE_CXX_FLAGS=-U__ARM_NEON__)
fi
if [[ -n "$abi" ]]; then
  ndk=${ANDROID_NDK_HOME:?ANDROID_NDK_HOME is required for Android builds}
  toolchain="$ndk/toolchains/llvm/prebuilt/linux-x86_64"
  triplet=${target%-*}-android
  # Rust's target and the NDK compiler prefix differ for arm64.
  case "$abi" in arm64-v8a) triplet=aarch64-linux-android ;; x86_64) triplet=x86_64-linux-android ;; esac
  export PATH="$toolchain/bin:$PATH"
  export CC="$toolchain/bin/${triplet}28-clang" CXX="$toolchain/bin/${triplet}28-clang++"
  export AR="$toolchain/bin/llvm-ar" RANLIB="$toolchain/bin/llvm-ranlib" STRIP="$toolchain/bin/llvm-strip" LD="$CC"
  cmake_args=(-DCMAKE_TOOLCHAIN_FILE="$ndk/build/cmake/android.toolchain.cmake" -DANDROID_ABI="$abi" -DANDROID_PLATFORM=28 -DANDROID_STL=c++_static)
  ffmpeg_args=(--target-os=android --arch="$arch" --enable-cross-compile --sysroot="$toolchain/sysroot")
  cpp_libs='-lc++_static -lc++abi -lm -ldl'
fi
export PKG_CONFIG="$pkg_config" PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig" PKG_CONFIG_PATH="$prefix/lib/pkgconfig"
if [[ "$arch" == x86_64 && -z "$darwin" ]]; then
  AS=$(command -v yasm || command -v nasm || true)
  if [[ -z "$AS" ]]; then
    AS="${ANDROID_NDK_HOME:-$HOME/Android/Sdk/ndk/28.2.13676358}/toolchains/llvm/prebuilt/linux-x86_64/bin/yasm"
  fi
  if [[ ! -x "$AS" ]]; then echo "Install nasm or yasm to build libvpx" >&2; exit 1; fi
  export AS
fi
cmake -S "$base/sources/rlottie" -B "$work/rlottie" -G Ninja ${cmake_args[@]+"${cmake_args[@]}"} ${rlottie_args[@]+"${rlottie_args[@]}"} \
  -DCMAKE_POLICY_VERSION_MINIMUM=3.5 -DCMAKE_POSITION_INDEPENDENT_CODE=ON -DBUILD_SHARED_LIBS=OFF \
  -DCMAKE_BUILD_TYPE=Release -DLOTTIE_MODULE=OFF -DLOTTIE_THREAD=OFF -DLOTTIE_CACHE=OFF \
  -DCMAKE_INSTALL_PREFIX="$prefix" -DLIB_INSTALL_DIR="$prefix/lib"
cmake --build "$work/rlottie" --target install --parallel 4
printf '\nLibs.private: %s\n' "$cpp_libs" >> "$prefix/lib/pkgconfig/rlottie.pc"
if [[ -n "$abi" ]]; then
  # Do not add the NDK's root library directory: it contains static Bionic libc.
  cp "$toolchain/sysroot/usr/lib/$triplet/libc++_static.a" "$prefix/lib/"
  cp "$toolchain/sysroot/usr/lib/$triplet/libc++abi.a" "$prefix/lib/"
fi
cmake -S "$base/sources/webp" -B "$work/webp" -G Ninja ${cmake_args[@]+"${cmake_args[@]}"} \
  -DCMAKE_POSITION_INDEPENDENT_CODE=ON -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
  -DWEBP_BUILD_ANIM_UTILS=OFF -DWEBP_BUILD_CWEBP=OFF -DWEBP_BUILD_DWEBP=OFF -DWEBP_BUILD_GIF2WEBP=OFF \
  -DWEBP_BUILD_IMG2WEBP=OFF -DWEBP_BUILD_VWEBP=OFF -DWEBP_BUILD_WEBPINFO=OFF -DWEBP_BUILD_WEBPMUX=OFF \
  -DCMAKE_INSTALL_PREFIX="$prefix"
cmake --build "$work/webp" --target install --parallel 4
mkdir -p "$work/vpx" "$work/ffmpeg"
cd "$work/vpx"
"$base/sources/vpx/configure" --target="$vpx_target" --prefix="$prefix" --enable-pic --disable-shared \
  --disable-examples --disable-tools --disable-docs --disable-unit-tests --disable-vp8-encoder --disable-vp9-encoder --disable-x86-asm
make -j4
make install
cd "$work/ffmpeg"
"$base/sources/ffmpeg/configure" ${ffmpeg_args[@]+"${ffmpeg_args[@]}"} --prefix="$prefix" --cc="${CC:-cc}" --cxx="${CXX:-c++}" \
  --ar="${AR:-ar}" --ranlib="${RANLIB:-ranlib}" --strip="${STRIP:-strip}" --pkg-config="$pkg_config" --pkg-config-flags=--static \
  --disable-everything --disable-autodetect --disable-programs --disable-doc --disable-debug \
  --disable-avdevice --disable-avfilter --disable-swresample --disable-network --disable-shared \
  --enable-static --enable-pic --enable-libvpx --enable-decoder=libvpx_vp8,libvpx_vp9 \
  --enable-demuxer=matroska --enable-parser=vp8,vp9 --enable-protocol=file --disable-x86asm
make -j4
make install
mkdir -p "$prefix/licenses"
cp "$base/sources/rlottie/COPYING" "$prefix/licenses/rlottie.txt"
cp "$base/sources/webp/COPYING" "$prefix/licenses/webp.txt"
cp "$base/sources/vpx/LICENSE" "$prefix/licenses/vpx.txt"
cp "$base/sources/ffmpeg/COPYING.LGPLv2.1" "$prefix/licenses/ffmpeg.txt"
printf '%s\n' "$stamp" > "$work/complete"
