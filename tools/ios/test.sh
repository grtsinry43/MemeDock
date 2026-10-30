#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
destination=${1:-"platform=iOS Simulator,name=iPhone 16,OS=18.4"}
bash "$root/tools/ios/build-bridge.sh" debug simulator
bash "$root/tools/ios/make-assets.sh"
results="$root/apps/ios/build/TestResults/$(date +%Y%m%d-%H%M%S)-$$.xcresult"
mkdir -p "$(dirname "$results")"
xcodebuild -project "$root/apps/ios/MemeDock.xcodeproj" -scheme MemeDock \
  -configuration Debug -destination "$destination" \
  -derivedDataPath "$root/apps/ios/build/DerivedData" \
  -resultBundlePath "$results" -parallel-testing-enabled NO \
  MEMEDOCK_IOS_PLATFORM=simulator test
