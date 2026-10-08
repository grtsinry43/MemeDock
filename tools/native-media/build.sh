#!/usr/bin/env bash
# Pinned, decoder-only media dependencies. All generated files stay in target/.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
target=${1:?Rust target required}
case "$target" in
  x86_64-unknown-linux-gnu) arch=x86_64; vpx_target=x86_64-linux-gcc; abi= ;;
  aarch64-linux-android) arch=aarch64; vpx_target=arm64-linux-gcc; abi=arm64-v8a ;;
  x86_64-linux-android) arch=x86_64; vpx_target=x86_64-linux-gcc; abi=x86_64 ;;
  *) echo "Unsupported native media target: $target" >&2; exit 1 ;;
esac
base="$root/target/native-media"
work="$base/$target"
prefix="$work/install"
mkdir -p "$work" "$base/sources"
exec 9>"$work/build.lock"
flock 9
stamp=$(sha256sum "$0" | cut -d ' ' -f 1)
if [[ -f "$work/complete" && $(cat "$work/complete") == "$stamp" && -f "$prefix/lib/libavcodec.a" ]]; then exit 0; fi
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
exec 8>"$base/sources.lock"
flock 8
fetch rlottie https://codeload.github.com/msrd0/rlottie-telegram/tar.gz/f435c0a4f364ce39f7e4eee9ddc7118573da9bf9 7cfcd8fad8d245d58b41631c7f9b45042a8dc80d3221541293612723f0b84f79
fetch webp https://storage.googleapis.com/downloads.webmproject.org/releases/webp/libwebp-1.6.0.tar.gz e4ab7009bf0629fd11982d4c2aa83964cf244cffba7347ecd39019a9e38c4564
fetch vpx https://codeload.github.com/webmproject/libvpx/tar.gz/refs/tags/v1.17.0 1020f184046187baa2985dbde38e0691f49c44088bca7a1842b0236c6081dc0a
fetch ffmpeg https://ffmpeg.org/releases/ffmpeg-9.0.1.tar.xz cf38e0e28c7e5605942c4a77755349b0145804a397af37eb1fb4c77cb237f635
flock -u 8
cmake_args=()
ffmpeg_args=()
cpp_libs=-lstdc++
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
export PKG_CONFIG=/usr/bin/pkg-config PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig" PKG_CONFIG_PATH="$prefix/lib/pkgconfig"
if [[ "$arch" == x86_64 ]]; then
  AS=$(command -v yasm || command -v nasm || true)
  if [[ -z "$AS" ]]; then
    AS="${ANDROID_NDK_HOME:-$HOME/Android/Sdk/ndk/28.2.13676358}/toolchains/llvm/prebuilt/linux-x86_64/bin/yasm"
  fi
  if [[ ! -x "$AS" ]]; then echo "Install nasm or yasm to build libvpx" >&2; exit 1; fi
  export AS
fi
cmake -S "$base/sources/rlottie" -B "$work/rlottie" -G Ninja "${cmake_args[@]}" \
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
cmake -S "$base/sources/webp" -B "$work/webp" -G Ninja "${cmake_args[@]}" \
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
"$base/sources/ffmpeg/configure" "${ffmpeg_args[@]}" --prefix="$prefix" --cc="${CC:-cc}" --cxx="${CXX:-c++}" \
  --ar="${AR:-ar}" --ranlib="${RANLIB:-ranlib}" --strip="${STRIP:-strip}" --pkg-config=/usr/bin/pkg-config --pkg-config-flags=--static \
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
