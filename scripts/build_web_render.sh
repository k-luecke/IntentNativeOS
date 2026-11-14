#!/usr/bin/env bash
set -euo pipefail
export PATH="${HOME}/.cargo/bin:${PATH}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}/userspace/apps/web_render"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -C target-feature=+sse,+sse2,-soft-float" cargo build --target x86_64-unknown-none --release
