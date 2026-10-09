#!/usr/bin/env bash
# Package the existing release binary and desktop integration, without compiling.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
version=$(python3 tools/release/version.py --check)
stage="$root/target/release-bundle/MemeDock-$version-linux-x86_64"
distribution="$root/target/distribution/linux"
if [[ -e "$stage" ]]; then
  printf '%s\n' "Bundle staging directory already exists: $stage" >&2
  exit 1
fi
install -Dm755 target/release/memedock "$stage/usr/bin/memedock"
install -Dm644 apps/linux/data/com.grtsinry43.memedock.desktop \
  "$stage/usr/share/applications/com.grtsinry43.memedock.desktop"
install -Dm644 apps/linux/data/icons/hicolor/scalable/apps/com.grtsinry43.memedock.svg \
  "$stage/usr/share/icons/hicolor/scalable/apps/com.grtsinry43.memedock.svg"
install -Dm644 LICENSE "$stage/usr/share/licenses/memedock/LICENSE"
for license in target/native-media/x86_64-unknown-linux-gnu/install/licenses/*.txt; do
  install -Dm644 "$license" "$stage/usr/share/licenses/memedock/$(basename "$license")"
done
mkdir -p "$distribution"
archive="MemeDock-$version-linux-x86_64.tar.zst"
tar --zstd --owner=0 --group=0 -cf "$distribution/$archive" -C "$stage" usr
(cd "$distribution" && sha256sum "$archive" > SHA256SUMS)
