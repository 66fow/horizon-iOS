#!/usr/bin/env bash
# Builds Horizon Mods, the iPhone app that picks native mods and has GitHub build Horizon with
# them (packaging/ios/native-mods.md), into an unsigned HorizonMods.ipa. A sideloading app signs
# it with the player's own Apple ID, as it does Horizon.ipa.
#
#   packaging/ios/builder/package.sh VERSION OWNER/REPO OUTDIR
#
# Needs macOS with Xcode (swiftc and the iPhone SDK).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
version=$1
repo=$2
out=$(mkdir -p "$3" && cd "$3" && pwd)

stage=$(mktemp -d)
app=$stage/Payload/HorizonMods.app
mkdir -p "$app"
xcrun --sdk iphoneos swiftc -target arm64-apple-ios16.0 -O -parse-as-library \
  -o "$app/HorizonMods" "$here"/Sources/*.swift
chmod 755 "$app/HorizonMods"
sed -e "s/@VERSION@/$version/g" -e "s#@REPO@#$repo#g" "$here/Info.plist" > "$app/Info.plist"
plutil -lint "$app/Info.plist"
printf 'APPL????' > "$app/PkgInfo"
sips -z 120 120 "$here/icon.png" --out "$app/AppIcon60x60@2x.png" >/dev/null
sips -z 180 180 "$here/icon.png" --out "$app/AppIcon60x60@3x.png" >/dev/null

rm -f "$out/HorizonMods.ipa"
(cd "$stage" && ditto -c -k --sequesterRsrc --keepParent Payload "$out/HorizonMods.ipa")
rm -rf "$stage"
echo "$out/HorizonMods.ipa"
