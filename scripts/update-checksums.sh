#!/usr/bin/env bash
# Regenerate docs/CHECKSUMS.md from downloaded artifacts
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
source "$SCRIPT_DIR/../configs/defaults.env" 2>/dev/null || true

CHECKSUMS_FILE="${REAGENTS_ROOT}/docs/CHECKSUMS.md"

cat > "$CHECKSUMS_FILE" <<'HEADER'
# agentReagents Checksums

SHA256 checksums for verifying artifact integrity.

## How to Use

Automated verification:
```bash
bash scripts/verify-setup.sh
```

Manual single-file check:
```bash
sha256sum <file> | diff - <(grep <file> docs/CHECKSUMS.md)
```

---

## Checksums

HEADER

COUNT=0
cd "$REAGENTS_ROOT"

for pattern in "images/cloud/*.img" "debs/**/*.deb" "isos/*.iso"; do
    # shellcheck disable=SC2086
    for f in $pattern; do
        if [ -f "$f" ]; then
            sha256sum "$f" >> "$CHECKSUMS_FILE"
            COUNT=$((COUNT + 1))
        fi
    done
done

echo "Updated $CHECKSUMS_FILE with $COUNT checksums"
