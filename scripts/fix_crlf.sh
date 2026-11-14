#!/usr/bin/env bash
# scripts/fix_crlf.sh
set -e
for f in scripts/*.sh userspace/*/*.sh userspace/*/*/*.sh tools/*/*.sh; do
    if [ -f "$f" ]; then
        sed -i 's/\r$//' "$f"
    fi
done
echo "Line ending normalization complete."
