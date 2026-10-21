#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -P -- "$(dirname -- "$0")" && pwd -P)
PREFIX=$(dirname -- "$(dirname -- "$ROOT")")
MANAGED=false
if [ -e "$PREFIX/.fairway-installation" ] || [ -L "$PREFIX/.fairway-installation" ]; then
  if [ ! -f "$PREFIX/.fairway-installation" ] || [ -L "$PREFIX/.fairway-installation" ] ||
     [ ! -d "$PREFIX/releases" ] || [ -L "$PREFIX/releases" ] || [ ! -L "$PREFIX/current" ] ||
     [ "$(dirname -- "$ROOT")" != "$PREFIX/releases" ]; then
    echo 'Installation layout mismatch; files preserved.' >&2
    exit 1
  fi
  # Older custom prefixes may contain coursework or data. Keep those siblings,
  # and refuse deletion if the payload itself overlaps a protected user path.
  case "${XDG_DATA_HOME:-}" in
    /*) DATA_HOME=$XDG_DATA_HOME ;;
    *) DATA_HOME="$HOME/.local/share" ;;
  esac
  for PROTECTED in "$HOME/fairway/www" "$DATA_HOME/dev.fairway.environment"; do
    PROTECTED=$(realpath -m -- "$PROTECTED")
    case "$PREFIX/releases/" in "$PROTECTED/"*) echo 'Installation files overlap your projects or application data; files preserved.' >&2; exit 1 ;; esac
    case "$PROTECTED/" in "$PREFIX/releases/"*) echo 'Installation files overlap your projects or application data; files preserved.' >&2; exit 1 ;; esac
  done
  MANAGED=true
fi
"$ROOT/bin/fairway" shutdown
"$ROOT/bin/fairway" clear-domains
"$ROOT/bin/fairway" clear-https-trust
"$ROOT/bin/fairway" remove-cli
"$ROOT/bin/fairway" shutdown
if [ "$MANAGED" = true ]; then
  rm -rf -- "$PREFIX/releases"
  rm -f -- "$PREFIX/current" "$PREFIX/.fairway-installation"
  if ! rmdir -- "$PREFIX" 2>/dev/null; then
    # Retain ownership recognition so reinstalling into this nonempty prefix
    # works without accepting arbitrary existing directories in the installer.
    printf '%s\n' 'Fairway XMP user installation' > "$PREFIX/.fairway-installation"
    printf 'Remaining files have been kept in %s\n' "$PREFIX"
  fi
else
  printf 'Shell integration removed. Remove this extracted directory manually: %s\n' "$ROOT"
fi
printf 'Fairway XMP removed. Your projects, database files and backups have been preserved.\n'
