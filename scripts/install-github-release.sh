#!/bin/sh
set -eu

repo=${GROK_BUILD_REPO:-victor-software-house/grok-build}
version=${GROK_BUILD_VERSION:-__RELEASE_VERSION__}
prefix=${GROK_BUILD_PREFIX:-"$HOME/.local"}
asset="grok-build-${version#v}-macos-arm64.tar.gz"
base_url=${GROK_BUILD_BASE_URL:-"https://github.com/${repo}/releases/download/${version}"}

if [ "$version" = __RELEASE_VERSION__ ]; then
  printf '%s\n' 'Installer release version was not populated.' >&2
  exit 1
fi
tmp=$(mktemp -d "${TMPDIR:-/tmp}/grok-build-install.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

if [ "$(uname -s)" != Darwin ] || [ "$(uname -m)" != arm64 ]; then
  printf '%s\n' 'This release supports macOS arm64 only.' >&2
  exit 1
fi

curl -fsSL "${base_url}/${asset}" -o "$tmp/$asset"
curl -fsSL "${base_url}/SHA256SUMS" -o "$tmp/SHA256SUMS"
expected=$(awk -v name="$asset" '$2 == name { print $1 }' "$tmp/SHA256SUMS")
[ -n "$expected" ] || {
  printf 'No checksum found for %s\n' "$asset" >&2
  exit 1
}
actual=$(shasum -a 256 "$tmp/$asset" | awk '{ print $1 }')
[ "$actual" = "$expected" ] || {
  printf 'Checksum mismatch for %s\n' "$asset" >&2
  exit 1
}

tar -xzf "$tmp/$asset" -C "$tmp"
root="$tmp/grok-build-${version#v}-macos-arm64"
file "$root/xai-grok-pager" | grep -q 'Mach-O 64-bit executable arm64'
install -d "$prefix/bin"
install -m 0755 "$root/xai-grok-pager" "$prefix/bin/xai-grok-pager"
ln -sfn xai-grok-pager "$prefix/bin/grok"
"$prefix/bin/grok" --version
printf 'Installed grok to %s/bin/grok\n' "$prefix"
