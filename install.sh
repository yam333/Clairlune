#!/bin/sh
# Installs Clairlune on a Mac, from its latest GitHub release:
#
#   curl -fsSL https://raw.githubusercontent.com/yam333/Clairlune/main/install.sh | sh
#
# It downloads the DMG for this Mac, checks it against the release's
# SHA256SUMS, checks the app's seal, puts Clairlune in Applications and opens
# it. It asks for no password and changes nothing else. To get a certain
# version, set CLAIRLUNE_VERSION (0.30.0, say); to not open it, CLAIRLUNE_OPEN=0.

set -eu

REPO="https://github.com/yam333/Clairlune"

say() { printf '%s\n' "$*"; }
fail() {
  printf 'clairlune: %s\n' "$*" >&2
  exit 1
}

tmp=""
mounted=""
staged=""
cleanup() {
  [ -z "$mounted" ] || hdiutil detach "$mounted" -force >/dev/null 2>&1 || true
  [ -z "$staged" ] || rm -rf "$staged"
  [ -z "$tmp" ] || rm -rf "$tmp"
}

fetch() {
  curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$2" "$1" || fail "couldn't download $1"
}

main() {
  [ "$(uname -s)" = Darwin ] || fail "this installs the Mac app; for other systems see $REPO#install"
  macos=$(sw_vers -productVersion)
  [ "${macos%%.*}" -ge 13 ] || fail "Clairlune needs macOS 13 or later (this is $macos)"

  # A Terminal running under Rosetta reports an Intel machine.
  if [ "$(uname -m)" = arm64 ] || [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then
    arch=aarch64
  else
    arch=x64
  fi

  version=${CLAIRLUNE_VERSION:-}
  version=${version#v}
  if [ -z "$version" ]; then
    # "latest" redirects to .../releases/tag/v<version>.
    latest=$(curl --proto '=https' --tlsv1.2 -fsSLI -o /dev/null -w '%{url_effective}' "$REPO/releases/latest") ||
      fail "couldn't reach $REPO"
    version=${latest##*/v}
  fi
  case $version in
    "" | *[!0-9A-Za-z.-]*) fail "'$version' isn't a version" ;;
  esac

  trap cleanup EXIT
  trap 'exit 1' HUP INT TERM
  tmp=$(mktemp -d)

  dmg="Clairlune_${version}_${arch}.dmg"
  say "Downloading Clairlune $version"
  fetch "$REPO/releases/download/v$version/$dmg" "$tmp/$dmg"
  fetch "$REPO/releases/download/v$version/SHA256SUMS" "$tmp/SHA256SUMS"

  want=$(awk -v f="$dmg" '$2 == f || $2 == "*" f { print $1 }' "$tmp/SHA256SUMS")
  got=$(shasum -a 256 "$tmp/$dmg" | awk '{ print $1 }')
  [ -n "$want" ] || fail "the release's SHA256SUMS doesn't list $dmg"
  [ "$want" = "$got" ] || fail "the download doesn't match the release's checksum; nothing was installed"

  mkdir "$tmp/mnt"
  hdiutil attach "$tmp/$dmg" -mountpoint "$tmp/mnt" -nobrowse -readonly -quiet || fail "couldn't open the DMG"
  mounted="$tmp/mnt"
  codesign --verify --deep --strict "$mounted/Clairlune.app" || fail "the app's seal is broken; nothing was installed"

  # Over a copy that's already there (which may be running: it starts itself
  # again from the new one), else in Applications, or the account's own where
  # that takes an administrator.
  if [ -d "$HOME/Applications/Clairlune.app" ] && [ ! -d /Applications/Clairlune.app ]; then
    dir="$HOME/Applications"
  elif [ -w /Applications ]; then
    dir=/Applications
  else
    dir="$HOME/Applications"
  fi
  mkdir -p "$dir"
  app="$dir/Clairlune.app"

  # Copied beside the old one, then swapped in, so a copy that stops halfway
  # leaves the old one as it was.
  staged="$dir/.Clairlune-$$"
  mkdir "$staged"
  ditto "$mounted/Clairlune.app" "$staged/Clairlune.app"
  if [ -d "$app" ]; then
    mv "$app" "$staged/old.app" || fail "couldn't replace $app"
  fi
  if ! mv "$staged/Clairlune.app" "$app"; then
    [ ! -d "$staged/old.app" ] || mv "$staged/old.app" "$app"
    fail "couldn't put Clairlune in $dir"
  fi

  say "Installed $app"
  if [ "${CLAIRLUNE_OPEN:-1}" != 0 ]; then
    open "$app"
    say "It's in the menu bar (the moon)."
  fi
}

main "$@"
