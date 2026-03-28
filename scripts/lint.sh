#!/usr/bin/env bash
# agentReagents lint — syntax check all active scripts
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PASS=0
FAIL=0

echo "Checking active scripts..."
for script in "$SCRIPT_DIR"/*.sh; do
    name="$(basename "$script")"
    if bash -n "$script" 2>/dev/null; then
        echo "  PASS: $name"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $name"
        bash -n "$script"
        FAIL=$((FAIL + 1))
    fi
done

echo ""
echo "Checking legacy scripts..."
if [ -d "$SCRIPT_DIR/legacy" ]; then
    for script in "$SCRIPT_DIR"/legacy/*.sh; do
        name="legacy/$(basename "$script")"
        if bash -n "$script" 2>/dev/null; then
            echo "  PASS: $name"
            PASS=$((PASS + 1))
        else
            echo "  FAIL: $name"
            FAIL=$((FAIL + 1))
        fi
    done
fi

echo ""
if command -v shellcheck &>/dev/null; then
    echo "Running shellcheck on active scripts..."
    for script in "$SCRIPT_DIR"/*.sh; do
        name="$(basename "$script")"
        if shellcheck -S warning "$script" 2>/dev/null; then
            echo "  PASS: $name"
        else
            echo "  WARN: $name (shellcheck warnings)"
        fi
    done
    echo ""
fi

echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
