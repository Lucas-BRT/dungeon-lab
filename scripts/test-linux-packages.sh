#!/usr/bin/env bash
# Checks what scripts/package-linux.sh, `cargo deb` and `cargo generate-rpm` made, and runs install.sh against them as if they were a
# release (no network, nothing outside a temporary directory):
#   scripts/test-linux-packages.sh [DIST_DIR]     (default: dist)
set -euo pipefail
cd "$(dirname "$0")/.."

dist=$(realpath "${1:-dist}")
ok() { printf 'ok: %s\n' "$*"; }
die() { echo "FAIL: $*" >&2; exit 1; }
has() { grep -q -- "$1" <<<"$2" || die "$3"; }

tarball=$dist/tapiti-x86_64-unknown-linux-gnu.tar.gz
deb=$(ls "$dist"/tapiti_*_amd64.deb)
rpm=$(ls "$dist"/tapiti-*.x86_64.rpm 2>/dev/null || true)
[ -f "$tarball" ] || die "$tarball is missing"

command -v desktop-file-validate >/dev/null && desktop-file-validate apps/desktop/packaging/tapiti.desktop && ok "the launcher entry is valid"

files=$(dpkg-deb -c "$deb")
for f in usr/bin/tapiti usr/share/applications/tapiti.desktop usr/share/icons/hicolor/256x256/apps/tapiti.png; do
    has "./$f" "$files" "the .deb has no $f"
done
ok "the .deb has the program, the launcher entry and the icon ($(dpkg-deb -f "$deb" Version))"

if [ -n "$rpm" ] && command -v rpm >/dev/null; then
    files=$(rpm -qpl "$rpm")
    for f in /usr/bin/tapiti /usr/share/applications/tapiti.desktop /usr/share/icons/hicolor/256x256/apps/tapiti.png /usr/share/licenses/tapiti/LICENSE; do
        has "$f" "$files" "the .rpm has no $f"
    done
    ok "the .rpm has the program, the launcher entry, the icon and the licence"
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/release/latest/download"
cp "$tarball" "$work/release/latest/download/"
(cd "$work/release/latest/download" && sha256sum "$(basename "$tarball")" >SHA256SUMS)
export TAPITI_RELEASE_BASE=file://$work/release
prefix=$work/prefix

./install.sh --prefix "$prefix" >/dev/null
[ -x "$prefix/bin/tapiti" ] || die "install.sh did not install the program"
grep -q "^Exec=$prefix/bin/tapiti\$" "$prefix/share/applications/tapiti.desktop" || die "the launcher entry does not start the installed program"
ok "install.sh installs"
./install.sh --prefix "$prefix" --update | grep -q "already the latest" || die "--update reinstalled the same version"
ok "install.sh --update leaves the same version alone"
echo "0000  $(basename "$tarball")" >"$work/release/latest/download/SHA256SUMS"
if ./install.sh --prefix "$work/other" >/dev/null 2>&1; then die "install.sh accepted a download that does not match SHA256SUMS"; fi
ok "install.sh refuses a download that does not match SHA256SUMS"
./install.sh --prefix "$prefix" --uninstall >/dev/null
[ -z "$(find "$prefix" -type f)" ] || die "--uninstall left files behind"
ok "install.sh --uninstall removes what it installed"
