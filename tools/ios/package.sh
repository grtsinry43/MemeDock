#!/usr/bin/env bash
# Package an existing unsigned device Release build for users to sign locally.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
version=${1:?Release version required}
bundle="$root/apps/ios/build/DerivedData/Build/Products/Release-iphoneos/MemeDock.app"
plist="$bundle/Info.plist"
plutil -lint "$plist"
test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$plist")" = "$version"
test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleSupportedPlatforms:0' "$plist")" = iPhoneOS
test "$(lipo -archs "$bundle/MemeDock")" = arm64
# Simulator binaries also have an arm64 slice; verify the Mach-O platform too.
xcrun vtool -show-build "$bundle/MemeDock" | grep -Eq 'platform[[:space:]]+IOS$'
if [[ -d "$bundle/_CodeSignature" || -f "$bundle/embedded.mobileprovision" ]]; then
  echo "Expected an unsigned build without a provisioning profile: $bundle" >&2
  exit 1
fi
distribution="$root/target/distribution/ios"
mkdir -p "$distribution"
staging=$(mktemp -d "${TMPDIR:-/tmp}/memedock-ipa.XXXXXX")
trap 'rm -rf "$staging"' EXIT
mkdir -p "$staging/Payload"
ditto "$bundle" "$staging/Payload/MemeDock.app"
ipa="$distribution/MemeDock-$version-ios-arm64-unsigned.ipa"
ditto -c -k --keepParent "$staging/Payload" "$ipa"
unzip -tq "$ipa"
unzip -Z1 "$ipa" | grep -Fx 'Payload/MemeDock.app/MemeDock'
cd "$distribution"
shasum -a 256 "$(basename "$ipa")" > SHA256SUMS
