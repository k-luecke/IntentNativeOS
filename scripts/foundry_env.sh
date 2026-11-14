#!/usr/bin/env bash
# scripts/foundry_env.sh - Environment wrapper for INOS Autonomous Product Foundry
export HOME=$HOME
export PATH="$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin"
exec "$@"
