#!/bin/sh
set -eu
umask 077
[ "$(uname -s)" = Linux ] && [ "$(uname -m)" = x86_64 ] || { echo 'This installer requires Linux x86_64.' >&2; exit 1; }
[ "$(id -u)" != 0 ] || { echo 'Install as your normal user, without sudo.' >&2; exit 1; }
GLIBC=$(getconf GNU_LIBC_VERSION 2>/dev/null || true)
case "$GLIBC" in
  'glibc '*) VERSION=${GLIBC#glibc }; MAJOR=${VERSION%%.*}; MINOR=${VERSION#*.}; MINOR=${MINOR%%.*}
    [ "$MAJOR" -gt 2 ] || { [ "$MAJOR" -eq 2 ] && [ "$MINOR" -ge 35 ]; } || { echo 'glibc 2.35 or newer is required.' >&2; exit 1; } ;;
  *) echo 'A glibc-based Linux distribution is required; Alpine/musl is not supported.' >&2; exit 1 ;;
esac
PREFIX="$HOME/.local/share/fairway-xmp"
if [ "$#" -gt 0 ]; then
  [ "$#" -eq 2 ] && [ "$1" = --prefix ] || { echo 'Usage: installer.run [--prefix /absolute/path]' >&2; exit 1; }
  PREFIX=$2
fi
case "$PREFIX" in /*) ;; *) echo 'The installation path must be absolute.' >&2; exit 1 ;; esac
[ "$PREFIX" != / ] && [ "$PREFIX" != "$HOME" ] || { echo 'Choose a dedicated installation directory.' >&2; exit 1; }
[ ! -L "$PREFIX" ] || { echo 'The installation directory must not be a symlink.' >&2; exit 1; }
# Resolve existing symlink ancestors and missing path components before comparing
# paths. A dedicated prefix must not contain, or live inside, student storage.
PREFIX=$(realpath -m -- "$PREFIX")
[ "$PREFIX" != / ] && [ "$PREFIX" != "$(realpath -m -- "$HOME")" ] || { echo 'Choose a dedicated installation directory.' >&2; exit 1; }
case "${XDG_DATA_HOME:-}" in
  /*) DATA_HOME=$XDG_DATA_HOME ;;
  *) DATA_HOME="$HOME/.local/share" ;;
esac
for PROTECTED in "$HOME/fairway/www" "$DATA_HOME/dev.fairway.environment"; do
  PROTECTED=$(realpath -m -- "$PROTECTED")
  case "$PREFIX/" in "$PROTECTED/"*) echo 'Choose an installation directory separate from your projects and application data.' >&2; exit 1 ;; esac
  case "$PROTECTED/" in "$PREFIX/"*) echo 'Choose an installation directory separate from your projects and application data.' >&2; exit 1 ;; esac
done
if [ -e "$PREFIX" ] && [ ! -f "$PREFIX/.fairway-installation" ]; then
  echo 'This directory already exists and is not a Fairway installation. Choose a new --prefix.' >&2; exit 1
fi
if [ -L "$PREFIX/.fairway-installation" ] || [ -L "$PREFIX/releases" ] ||
   { [ -e "$PREFIX/releases" ] && [ ! -d "$PREFIX/releases" ]; } ||
   { [ -e "$PREFIX/current" ] && [ ! -L "$PREFIX/current" ]; }; then
  echo 'Installation layout mismatch; files preserved.' >&2; exit 1
fi
SOURCE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
BUILD=$(cat "$SOURCE/BUILD")
case "$BUILD" in *[!a-zA-Z0-9._-]*|'') echo 'Invalid build identifier.' >&2; exit 1 ;; esac
OLD_CURRENT=$(readlink "$PREFIX/current" 2>/dev/null || true)
if [ -x "$PREFIX/current/bin/fairway" ]; then
  # Back up existing student databases before switching runtime files. Failure aborts the upgrade.
  "$PREFIX/current/bin/fairway" __prepare-update
fi
mkdir -p "$PREFIX/releases"
if [ ! -d "$PREFIX/releases/$BUILD" ]; then
  STAGE=$(mktemp -d "$PREFIX/releases/.stage.XXXXXX")
  trap 'rm -rf "$STAGE"' EXIT HUP INT TERM
  cp -R "$SOURCE/." "$STAGE/"
  mv "$STAGE" "$PREFIX/releases/$BUILD"
  trap - EXIT HUP INT TERM
fi
ln -s "releases/$BUILD" "$PREFIX/.current-$$"
mv -Tf "$PREFIX/.current-$$" "$PREFIX/current"
printf '%s\n' 'Fairway XMP user installation' > "$PREFIX/.fairway-installation"
if ! "$PREFIX/current/bin/fairway" install-cli; then
  if [ -n "$OLD_CURRENT" ]; then
    ln -s "$OLD_CURRENT" "$PREFIX/.rollback-$$"
    mv -Tf "$PREFIX/.rollback-$$" "$PREFIX/current"
    "$PREFIX/current/bin/fairway" install-cli || true
  fi
  echo 'Installation setup failed; existing data is preserved.' >&2
  exit 1
fi
printf '\nInstalled Fairway XMP in %s\nReopen your shell, then run: fairway\nOr run now: "%s/current/bin/fairway"\nServices remain stopped until you start them.\n' "$PREFIX" "$PREFIX"
