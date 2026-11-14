#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$ROOT_DIR/models/pretrained"

echo "[1/2] Compiling C Llama-2 inference runner (models/pretrained/run.c)..."
cc -O3 -o "$ROOT_DIR/models/pretrained/run" "$ROOT_DIR/models/pretrained/run.c" -lm

WEIGHTS="$ROOT_DIR/models/pretrained/stories15M.bin"
EXPECTED_SHA="cd590644d963867a2b6e5a1107f51fad663c41d79c149fbecbbb1f95fa81f49a"
if [ ! -f "$WEIGHTS" ]; then
  echo "[2/3] Downloading TinyStories-15M weights (60.8 MB, MIT license)..."
  curl -fL "https://huggingface.co/karpathy/tinyllamas/resolve/main/stories15M.bin" -o "$WEIGHTS"
fi
echo "${EXPECTED_SHA}  ${WEIGHTS}" | sha256sum -c -

WEIGHTS42="$ROOT_DIR/models/pretrained/stories42M.bin"
if [ ! -f "$WEIGHTS42" ]; then
  echo "[3/3] Downloading TinyStories-42M weights (167 MB, MIT license)..."
  curl -fL "https://huggingface.co/karpathy/tinyllamas/resolve/main/stories42M.bin" -o "$WEIGHTS42"
fi
echo "Model runner and weights verified."
