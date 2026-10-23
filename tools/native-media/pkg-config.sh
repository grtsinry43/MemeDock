#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
case " $* " in
  *rlottie*|*libwebp*|*libavformat*|*libavcodec*|*libavutil*|*libavfilter*|*libswscale*)
    target=${TARGET:-${HOST:-$(rustc -vV | sed -n 's/^host: //p')}}
    # pkg-config is invoked by the native crates before core's own build script.
    # Keep its stdout exclusively for pkg-config's machine-readable response.
    # The cached build is usually current, so probe it without taking the lock;
    # concurrent queries then never queue behind a finished build.
    if ! bash "$root/tools/native-media/build.sh" --verify "$target" >/dev/null 2>&1; then
      bash "$root/tools/native-media/build.sh" "$target" >&2
    fi
    export PKG_CONFIG_LIBDIR="$root/target/native-media/$target/install/lib/pkgconfig"
    export PKG_CONFIG_PATH="$PKG_CONFIG_LIBDIR"
    ;;
esac
# Darwin ships no /usr/bin/pkg-config, so fall back to the Homebrew one.
real=/usr/bin/pkg-config
if [[ ! -x "$real" ]]; then
  for candidate in /opt/homebrew/bin/pkg-config /usr/local/bin/pkg-config; do
    if [[ -x "$candidate" ]]; then real="$candidate"; break; fi
  done
fi
if [[ ! -x "$real" ]]; then
  echo "pkg-config is required; install it before building native media" >&2
  exit 1
fi
exec "$real" "$@"
