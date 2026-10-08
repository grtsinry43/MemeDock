#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
case " $* " in
  *rlottie*|*libwebp*|*libavformat*|*libavcodec*|*libavutil*|*libswscale*)
    target=${TARGET:-${HOST:-$(rustc -vV | sed -n 's/^host: //p')}}
    # pkg-config is invoked by the native crates before core's own build script.
    # Keep its stdout exclusively for pkg-config's machine-readable response.
    bash "$root/tools/native-media/build.sh" "$target" >&2
    export PKG_CONFIG_LIBDIR="$root/target/native-media/$target/install/lib/pkgconfig"
    export PKG_CONFIG_PATH="$PKG_CONFIG_LIBDIR"
    ;;
esac
exec /usr/bin/pkg-config "$@"
