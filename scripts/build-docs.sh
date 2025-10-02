#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
OUTPUT_DIR=${1:-site}
TARGET_DIR="$ROOT_DIR/$OUTPUT_DIR"

if ! command -v pandoc >/dev/null 2>&1; then
  echo "pandoc is required but was not found in PATH." >&2
  echo "Install it via 'brew install pandoc' or 'sudo apt-get install pandoc'." >&2
  exit 1
fi

mkdir -p "$TARGET_DIR"

pandoc "$ROOT_DIR/DOCS.md" \
  --standalone \
  --metadata title="syncx Documentation" \
  --output "$TARGET_DIR/index.html"

echo "Docs written to $TARGET_DIR/index.html"
