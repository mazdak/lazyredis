#!/usr/bin/env bash
set -euo pipefail

OWNER=${GITHUB_OWNER:-mazdak}
REPO=${GITHUB_REPO:-lazyredis}
TAP_REPO=${TAP_REPO:-../homebrew-tap}
FORMULA_PATH=${FORMULA_PATH:-$TAP_REPO/Formula/lazyredis.rb}
RELEASE_TAG=${LAZYREDIS_RELEASE_TAG:-${GITHUB_REF_NAME:-}}

if [[ -z "$RELEASE_TAG" ]]; then
  echo "error: LAZYREDIS_RELEASE_TAG is required (e.g. v0.8.0)" >&2
  exit 1
fi

VERSION=${RELEASE_TAG#v}

if [[ ! -d "$TAP_REPO" ]]; then
  echo "error: homebrew-tap repository not found at $TAP_REPO" >&2
  exit 1
fi

mkdir -p "$(dirname "$FORMULA_PATH")"

sha_for_target() {
  local target="$1"
  local asset="lazyredis-${RELEASE_TAG}-${target}.tar.gz"
  local url="https://github.com/${OWNER}/${REPO}/releases/download/${RELEASE_TAG}/${asset}"
  local tmp_file

  tmp_file=$(mktemp)
  curl -fsSL "$url" -o "$tmp_file"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$tmp_file" | awk '{print $1}'
  else
    shasum -a 256 "$tmp_file" | awk '{print $1}'
  fi
  rm -f "$tmp_file"
}

SHA_MAC_ARM=$(sha_for_target "aarch64-apple-darwin")
SHA_MAC_INTEL=$(sha_for_target "x86_64-apple-darwin")
SHA_LINUX_ARM=$(sha_for_target "aarch64-unknown-linux-gnu")
SHA_LINUX_INTEL=$(sha_for_target "x86_64-unknown-linux-gnu")

cat <<FORMULA > "$FORMULA_PATH"
class Lazyredis < Formula
  desc "A fast and lazy Terminal UI for inspecting and manipulating Redis databases"
  homepage "https://github.com/mazdak/lazyredis"
  version "${VERSION}"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/mazdak/lazyredis/releases/download/${RELEASE_TAG}/lazyredis-${RELEASE_TAG}-aarch64-apple-darwin.tar.gz"
      sha256 "${SHA_MAC_ARM}"
    end

    on_intel do
      url "https://github.com/mazdak/lazyredis/releases/download/${RELEASE_TAG}/lazyredis-${RELEASE_TAG}-x86_64-apple-darwin.tar.gz"
      sha256 "${SHA_MAC_INTEL}"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/mazdak/lazyredis/releases/download/${RELEASE_TAG}/lazyredis-${RELEASE_TAG}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "${SHA_LINUX_ARM}"
    end

    on_intel do
      url "https://github.com/mazdak/lazyredis/releases/download/${RELEASE_TAG}/lazyredis-${RELEASE_TAG}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "${SHA_LINUX_INTEL}"
    end
  end

  def install
    bin.install "lazyredis"
  end

  test do
    system "#{bin}/lazyredis", "--version"
  end
end
FORMULA

echo "Updated Homebrew formula at $FORMULA_PATH"
