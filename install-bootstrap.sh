#!/bin/sh
set -eu

# Remote bootstrap for `curl -fsSL https://install.tuenel.com | sh`.
# Keep the full installer in the release archive so local clone installs and
# remote installs execute exactly the same deployment code.

command -v curl >/dev/null 2>&1 || {
  echo "curl is required to install Tuenel." >&2
  exit 1
}
command -v tar >/dev/null 2>&1 || {
  echo "tar is required to install Tuenel." >&2
  exit 1
}

version=${TUENEL_VERSION:-0.4.12}
case "$version" in
  v*) release_tag=$version ;;
  *) release_tag="v$version" ;;
esac
case "$release_tag" in
  v[0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "TUENEL_VERSION must be a semantic version such as 0.4.12." >&2; exit 2 ;;
esac

temp_dir=$(mktemp -d "${TMPDIR:-/tmp}/tuenel-install.XXXXXX")
cleanup() { rm -rf "$temp_dir"; }
trap cleanup EXIT HUP INT TERM

archive="$temp_dir/tuenel.tar.gz"
curl --fail --location --silent --show-error --retry 3 \
  "https://codeload.github.com/mirrabase/tuenel/tar.gz/refs/tags/$release_tag" \
  --output "$archive"
tar -xzf "$archive" -C "$temp_dir"

source_dir=$(find "$temp_dir" -mindepth 1 -maxdepth 1 -type d -name 'tuenel-*' -print -quit)
if [ -z "$source_dir" ] || [ ! -x "$source_dir/install.sh" ]; then
  echo "The Tuenel $release_tag archive is missing install.sh." >&2
  exit 1
fi

# A pipe makes stdin non-interactive. Reattach the terminal when available so
# `curl ... | sh` without arguments still opens the normal installer wizard.
if [ "$#" -eq 0 ] && [ -r /dev/tty ] && [ -t 1 ]; then
  exec "$source_dir/install.sh" </dev/tty
fi
exec "$source_dir/install.sh" "$@"
