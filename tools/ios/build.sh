#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
configuration=${1:-debug}
platform=${2:-simulator}
case "$configuration" in
  debug) xcode_configuration=Debug ;;
  release) xcode_configuration=Release ;;
  *) echo "Usage: bash $0 [debug|release] [simulator|device]" >&2; exit 1 ;;
esac
case "$platform" in
  simulator) destination="generic/platform=iOS Simulator" ;;
  device) destination="generic/platform=iOS" ;;
  *) echo "Usage: bash $0 [debug|release] [simulator|device]" >&2; exit 1 ;;
esac
bash "$root/tools/ios/build-bridge.sh" "$configuration"
bash "$root/tools/ios/make-assets.sh"
# Device compilation here is deliberately unsigned. Xcode Run uses the local
# Team override instead; this command never creates profiles or certificates.
xcodebuild -project "$root/apps/ios/MemeDock.xcodeproj" -scheme MemeDock \
  -configuration "$xcode_configuration" -destination "$destination" \
  -derivedDataPath "$root/apps/ios/build/DerivedData" \
  CODE_SIGNING_ALLOWED=NO build
