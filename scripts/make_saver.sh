#!/bin/sh
# Builds Clairlune.saver, the macOS screen saver (crates/saver), into a folder:
#   scripts/make_saver.sh <rust target> <out dir> <version>
# The Release workflow puts it in Clairlune.app/Contents/Resources, where the
# app installs it from (Settings > General > Screen saver). Signed ad hoc, like
# the app: an Apple silicon Mac loads no unsigned code.
set -eu
target="$1"
out="$2"
version="$3"

cargo build --release -p clairlune-saver --target "$target"
saver="$out/Clairlune.saver"
rm -rf "$saver"
mkdir -p "$saver/Contents/MacOS" "$saver/Contents/Resources"
cp "target/$target/release/libclairlune_saver.dylib" "$saver/Contents/MacOS/Clairlune"
cat > "$saver/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleExecutable</key>
	<string>Clairlune</string>
	<key>CFBundleIdentifier</key>
	<string>app.clairlune.saver</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>Clairlune</string>
	<key>CFBundlePackageType</key>
	<string>BNDL</string>
	<key>CFBundleShortVersionString</key>
	<string>$version</string>
	<key>CFBundleVersion</key>
	<string>$version</string>
	<key>CFBundleSupportedPlatforms</key>
	<array>
		<string>MacOSX</string>
	</array>
	<key>LSMinimumSystemVersion</key>
	<string>13.0</string>
	<key>NSPrincipalClass</key>
	<string>ClairluneSaverView</string>
</dict>
</plist>
EOF
printf '%s\n' "$version" > "$saver/Contents/Resources/version"
codesign --force --sign - "$saver"
codesign --verify --strict --verbose=2 "$saver"
