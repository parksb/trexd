#!/bin/sh
set -eu

INSTALL_DIR=${INSTALL_DIR:-/usr/local/bin}

repository=https://github.com/parksb/trexd
asset=
work_dir=
staged_binary=

usage() {
  echo "Usage: sudo sh install.sh [latest|vX.Y.Z]"
  echo "Set INSTALL_DIR to choose the installation directory (default: /usr/local/bin)."
}

fail() {
  echo "Error: $*" >&2
  exit 1
}

is_version() {
  printf '%s\n' "$1" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$'
}

check_requirements() {
  if [ "$(uname -s)" != Linux ]; then
    fail "Only Linux is supported."
  fi

  if [ "$(id -u)" -ne 0 ]; then
    fail "Run this script with sudo."
  fi

  case "$INSTALL_DIR" in
    /*) ;;
    *) fail "INSTALL_DIR must be an absolute path." ;;
  esac

  for command in curl sha256sum install mktemp getconf grep mkdir; do
    if ! command -v "$command" >/dev/null; then
      fail "Missing command: $command"
    fi
  done

  if [ "$(getconf LONG_BIT)" != 64 ]; then
    fail "A 64-bit operating system is required."
  fi
}

detect_target() {
  architecture=$(uname -m)

  case "$architecture" in
    x86_64|amd64)
      echo x86_64-unknown-linux-gnu
      ;;
    aarch64|arm64)
      echo aarch64-unknown-linux-gnu
      ;;
    *)
      fail "Unsupported architecture: $architecture"
      ;;
  esac
}

curl_https() {
  curl --fail --silent --show-error \
    --location \
    --proto '=https' --proto-redir '=https' \
    --tlsv1.2 \
    "$@"
}

resolve_version() {
  if [ "$1" != latest ]; then
    printf '%s\n' "$1"
    return
  fi

  # GitHub redirects this URL to /releases/tag/vX.Y.Z.
  release_url=$(curl_https \
    --output /dev/null \
    --write-out '%{url_effective}' \
    "$repository/releases/latest")

  latest_version=${release_url##*/}

  if ! is_version "$latest_version"; then
    fail "Could not resolve the latest release."
  fi

  printf '%s\n' "$latest_version"
}

cleanup() {
  rm -rf -- "$work_dir"

  if [ -n "$staged_binary" ]; then
    rm -f -- "$staged_binary"
  fi
}

download_release() {
  base_url=$repository/releases/download/$version
  echo "Downloading trexd $version..."

  for file in "$asset" SHA256SUMS; do
    curl_https \
      --output "$work_dir/$file" \
      "$base_url/$file"
  done
}

verify_binary() {
  (
    cd "$work_dir" || exit 1
    sha256sum --check --strict --ignore-missing SHA256SUMS
  )

  chmod 755 "$work_dir/$asset"
  "$work_dir/$asset" --help >/dev/null
}

install_binary() {
  mkdir -p "$INSTALL_DIR"

  staged_binary=$(mktemp "$INSTALL_DIR/.trexd.XXXXXX")
  install -m 755 "$work_dir/$asset" "$staged_binary"

  mv -f -- "$staged_binary" "$INSTALL_DIR/trexd"
  staged_binary=
}

main() {
  case "${1:-}" in
    --help|-h)
      usage
      return
      ;;
  esac

  if [ "$#" -gt 1 ]; then
    usage >&2
    exit 1
  fi

  version=${1:-latest}

  if [ "$version" != latest ] && ! is_version "$version"; then
    fail "Invalid version: $version"
  fi

  check_requirements

  target=$(detect_target)
  asset=trexd-$target

  work_dir=$(mktemp -d)
  trap cleanup 0
  trap 'exit 130' INT
  trap 'exit 143' TERM

  version=$(resolve_version "$version")
  download_release
  verify_binary
  install_binary

  echo "Installed trexd $version to $INSTALL_DIR/trexd."
  echo "Restart trexd to use the installed version."
}

main "$@"
