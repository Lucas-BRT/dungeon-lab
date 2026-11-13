#!/usr/bin/env bash
# Installs the Tapiti desktop app from its GitHub releases, without sudo.
#
#   curl -fsSL https://raw.githubusercontent.com/Lucas-BRT/tapiti/main/install.sh | bash
#
# Options (after `bash -s --` when piping):
#   --version X.Y.Z  install that release instead of the latest
#   --prefix DIR     install in DIR/bin and DIR/share (default: ~/.local)
#   --update         install the release only if it is not the version installed
#   --verify         also verify the build provenance (a GitHub attestation)
#                    with `gh attestation verify`; needs the GitHub CLI 2.49+
#   --uninstall      remove Tapiti (nothing else is kept: the app writes no
#                    configuration)
#
# The download is checked against the release's SHA256SUMS.
# Prefer your package manager? Each release also has a .deb and an .rpm:
# https://github.com/Lucas-BRT/tapiti/releases/latest
#
# TAPITI_RELEASE_BASE overrides where releases are downloaded from, and
# TAPITI_GETCONF is the command that reports the C library: both are for tests.
set -euo pipefail

BASE=${TAPITI_RELEASE_BASE:-https://github.com/Lucas-BRT/tapiti/releases}
ASSET=tapiti-x86_64-unknown-linux-gnu.tar.gz
PREFIX=$HOME/.local
VERSION=latest
UPDATE=0
UNINSTALL=0
VERIFY=0

# The oldest glibc the release runs on: the one of the Ubuntu 22.04 runner that builds it.
MIN_GLIBC=2.35

while [ $# -gt 0 ]; do
    case $1 in
        --version)
            VERSION=${2:?--version needs a value}
            VERSION=${VERSION#v}
            shift
            ;;
        --prefix)
            PREFIX=${2:?--prefix needs a directory}
            shift
            ;;
        --verify) VERIFY=1 ;;
        --update) UPDATE=1 ;;
        --uninstall) UNINSTALL=1 ;;
        -h | --help)
            sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "unknown option: $1" >&2
            exit 2
            ;;
    esac
    shift
done

say() { printf '==> %s\n' "$*"; }
die() {
    echo "error: $*" >&2
    exit 1
}

# What a release puts under the prefix: only these are ever replaced or removed.
remove_release() {
    rm -f "$PREFIX/bin/tapiti" \
        "$PREFIX/share/applications/tapiti.desktop" \
        "$PREFIX/share/icons/hicolor/256x256/apps/tapiti.png"
    rm -rf "${PREFIX:?}/share/tapiti" "${PREFIX:?}/share/doc/tapiti"
}

if [ "$UNINSTALL" = 1 ]; then
    remove_release
    say "removed Tapiti from $PREFIX"
    exit 0
fi

[ "$(uname -s)" = Linux ] || die "this script installs the Linux build; for Windows, download the .zip or the .exe from the releases page"
[ "$(uname -m)" = x86_64 ] || die "Tapiti's Linux release is built for x86_64 only"
for tool in curl tar gzip sha256sum; do
    command -v "$tool" >/dev/null || die "$tool is required"
done

libc=$(${TAPITI_GETCONF:-getconf} GNU_LIBC_VERSION 2>/dev/null || true)
case $libc in
    "glibc "*)
        found=${libc#glibc }
        if [ "$(printf '%s\n%s\n' "$MIN_GLIBC" "$found" | sort -V | head -n 1)" != "$MIN_GLIBC" ]; then
            die "Tapiti's release needs glibc $MIN_GLIBC or newer (Ubuntu 22.04+, Debian 12+, Fedora 36+...); this system has glibc $found."
        fi
        ;;
    *)
        die "Tapiti's release needs glibc $MIN_GLIBC or newer (Ubuntu 22.04+, Debian 12+, Fedora 36+...); this system does not use glibc (musl, Alpine?)."
        ;;
esac

if [ "$VERSION" = latest ]; then
    url=$BASE/latest/download
else
    url=$BASE/download/v$VERSION
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
say "downloading $url/$ASSET"
curl -fL --retry 3 -o "$work/$ASSET" "$url/$ASSET"
curl -fsL --retry 3 -o "$work/SHA256SUMS" "$url/SHA256SUMS"
(cd "$work" && grep " $ASSET\$" SHA256SUMS | sha256sum --check --quiet -) ||
    die "the download does not match the release's SHA256SUMS"
if [ "$VERIFY" = 1 ]; then
    command -v gh >/dev/null || die "--verify needs the GitHub CLI: https://cli.github.com"
    # `gh attestation` came with GitHub CLI 2.49; an older one would print its help and fail.
    gh attestation --help >/dev/null 2>&1 || die "--verify needs GitHub CLI 2.49 or newer (this is $(gh --version | head -n 1)): https://cli.github.com"
    say "verifying the build provenance"
    if ! attestation=$(gh attestation verify "$work/$ASSET" --repo Lucas-BRT/tapiti 2>&1); then
        printf '%s\n' "$attestation" >&2
        die "the build provenance of the download could not be verified"
    fi
fi

tar -xzf "$work/$ASSET" -C "$work"
src=$work/${ASSET%.tar.gz}
new=$(cat "$src/share/tapiti/VERSION")
old=$(cat "$PREFIX/share/tapiti/VERSION" 2>/dev/null || true)
if [ "$UPDATE" = 1 ] && [ "$new" = "$old" ]; then
    say "Tapiti $old is already the latest release"
    exit 0
fi

remove_release
mkdir -p "$PREFIX/bin" "$PREFIX/share/tapiti" "$PREFIX/share/doc" \
    "$PREFIX/share/applications" "$PREFIX/share/icons/hicolor/256x256/apps"
install -m 755 "$src/bin/tapiti" "$PREFIX/bin/tapiti"
install -m 644 "$src/share/tapiti/VERSION" "$PREFIX/share/tapiti/VERSION"
install -m 644 "$src/share/icons/hicolor/256x256/apps/tapiti.png" "$PREFIX/share/icons/hicolor/256x256/apps/tapiti.png"
cp -R "$src/share/doc/tapiti" "$PREFIX/share/doc/tapiti"
# The launcher starts the binary by its full path, so it works before $PREFIX/bin is on PATH.
sed "s|^Exec=tapiti\$|Exec=$PREFIX/bin/tapiti|" "$src/share/applications/tapiti.desktop" >"$PREFIX/share/applications/tapiti.desktop"
chmod 644 "$PREFIX/share/applications/tapiti.desktop"
say "installed Tapiti $new in $PREFIX"
[ "$UPDATE" = 1 ] && [ -n "$old" ] && say "updated from $old"

# Tells how to put $PREFIX/bin on PATH: the line for the user's shell and the
# file it goes in. The user's files are never edited.
path_note() {
    local dir=$PREFIX/bin file line
    case $dir in
        "$HOME"/*) dir=\$HOME/${dir#"$HOME"/} ;;
    esac
    case $(basename "${SHELL:-}") in
        bash) file=$HOME/.bashrc ;;
        zsh) file=${ZDOTDIR:-$HOME}/.zshrc ;;
        fish) file=$HOME/.config/fish/config.fish ;;
        *) file=$HOME/.profile ;;
    esac
    if [ "$(basename "${SHELL:-}")" = fish ]; then
        line="fish_add_path $dir"
    else
        # shellcheck disable=SC2016  # the $PATH is for the profile, not for this shell
        line="export PATH=\"$dir:"'$PATH"'
    fi
    case $file in
        "$HOME"/*) file=\~/${file#"$HOME"/} ;;
    esac
    echo "note: $PREFIX/bin is not on your PATH, so \`tapiti\` is not found in a terminal yet."
    echo "  Add this line to $file:"
    echo "    $line"
    echo "  Then open a new terminal, or run that line now. The launcher entry works already."
}

case ":$PATH:" in
    *":$PREFIX/bin:"*) ;;
    *) path_note ;;
esac

echo "Run it with \`tapiti\`, or from your application menu. It needs OpenGL 2 (a graphics driver; a VM without a GPU may not have one)."
