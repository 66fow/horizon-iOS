#!/usr/bin/env bash
# Packages the game for iOS: Horizon.app (the binary, Info.plist, the icon) in an unsigned
# Horizon.ipa. A sideloading tool (Sideloadly, AltStore) signs it with the player's own Apple ID
# when it installs it; with IOS_SIGN_IDENTITY set (an "Apple Development: …" identity) and
# IOS_PROVISIONING_PROFILE naming a .mobileprovision, it is signed here instead.
#
#   packaging/ios/package.sh BINARY VERSION OUTDIR
#
# Needs macOS (sips, plutil, ditto). Horizon ships none of Mojang's files: the player copies the
# prepared resources into the app's Documents folder with the Files app (packaging/ios/README.md).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
binary=$1
version=$2
out=$(mkdir -p "$3" && cd "$3" && pwd)
[[ "$(lipo -archs "$binary")" == "arm64" ]] || { echo "$binary is not an arm64 binary" >&2; exit 1; }

stage=$(mktemp -d)
app=$stage/Payload/Horizon.app
mkdir -p "$app"
cp "$binary" "$app/horizon-client"
chmod 755 "$app/horizon-client"
sed "s/@VERSION@/$version/g" "$here/Info.plist" > "$app/Info.plist"
plutil -lint "$app/Info.plist"
printf 'APPL????' > "$app/PkgInfo"
icon=$here/icon.png
sips -z 120 120 "$icon" --out "$app/AppIcon60x60@2x.png" >/dev/null
sips -z 180 180 "$icon" --out "$app/AppIcon60x60@3x.png" >/dev/null

if [[ -n "${IOS_SIGN_IDENTITY:-}" ]]; then
  cp "$IOS_PROVISIONING_PROFILE" "$app/embedded.mobileprovision"
  codesign --force --sign "$IOS_SIGN_IDENTITY" --timestamp=none "$app"
fi

rm -f "$out/Horizon.ipa"
(cd "$stage" && ditto -c -k --sequesterRsrc --keepParent Payload "$out/Horizon.ipa")
rm -rf "$stage"
echo "$out/Horizon.ipa"
