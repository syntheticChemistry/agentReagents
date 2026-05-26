#!/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# validate-lithoSpore-container.sh — Docker-based lithoSpore validation.
#
# Runs the full Tier 2 validation inside a clean Ubuntu 24.04 container
# with no pre-installed Rust toolchain. Proves the musl-static binaries
# work on a bare system. No libvirt/KVM required — only Docker.
#
# Phases:
#   0. Prerequisite check (tarball or usb-staging dir)
#   1. Build ephemeral container with lithoSpore artifact
#   2. Run self-test (artifact integrity)
#   3. Run tier detection
#   4. Run full Tier 2 validation (7 modules)
#   5. Run data integrity verification (BLAKE3)
#   6. Retrieve results
#   7. Generate deployment report
#
# Usage:
#   ./scripts/validate-lithoSpore-container.sh
#   ./scripts/validate-lithoSpore-container.sh --tarball /path/to/lithoSpore-usb.tar.gz
#   ./scripts/validate-lithoSpore-container.sh --airgap          # Drop network

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
LITHO_ROOT="${LITHO_ROOT:-$(realpath "$REPO_ROOT/../../gardens/lithoSpore")}"
TARBALL="${TARBALL:-}"
RESULTS_DIR="${RESULTS_DIR:-/tmp/lithoSpore-container-results}"
CONTAINER_NAME="lithoSpore-validate-$$"
CONTAINER_IMAGE="ubuntu:24.04"
AIRGAP=false
KEEP_CONTAINER=false

while [ $# -gt 0 ]; do
    case "$1" in
        --tarball)          TARBALL="$2"; shift 2 ;;
        --airgap)           AIRGAP=true; shift ;;
        --keep-container)   KEEP_CONTAINER=true; shift ;;
        --results-dir)      RESULTS_DIR="$2"; shift 2 ;;
        -h|--help)
            echo "Usage: $0 [--tarball PATH] [--airgap] [--keep-container] [--results-dir DIR]"
            exit 0
            ;;
        *)  echo "Unknown option: $1"; exit 1 ;;
    esac
done

cleanup() {
    if [ "$KEEP_CONTAINER" = false ]; then
        docker rm -f "$CONTAINER_NAME" 2>/dev/null || true
    fi
}
trap cleanup EXIT

echo ""
echo "=================================================================="
echo "  lithoSpore Container Validation"
echo "  Deployment pattern: Docker (${AIRGAP:+airgap, }no libvirt)"
echo "=================================================================="
echo ""

# ── Phase 0: Prerequisites ──────────────────────────────────────────
echo "=== Phase 0: Prerequisites ==="

if ! command -v docker &>/dev/null; then
    echo "ERROR: Docker not found. Install Docker to use container validation."
    echo "  For VM-based validation, use validate-lithoSpore.sh instead."
    exit 1
fi

if [ -z "$TARBALL" ]; then
    TARBALL="/tmp/lithoSpore-usb.tar.gz"
    if [ ! -f "$TARBALL" ]; then
        if [ -d "$LITHO_ROOT/usb-staging" ]; then
            echo "==> Building USB tarball from usb-staging/..."
            tar czf "$TARBALL" -C "$LITHO_ROOT/usb-staging" .
        else
            echo "ERROR: No USB artifact. Run:"
            echo "  cd $LITHO_ROOT && ./scripts/assemble-usb.sh --skip-python --skip-fetch"
            exit 1
        fi
    fi
fi

echo "  Tarball:    $TARBALL ($(du -h "$TARBALL" | cut -f1))"
echo "  Image:      $CONTAINER_IMAGE"
echo "  Airgap:     $AIRGAP"
echo "  Container:  $CONTAINER_NAME"
echo ""

# ── Phase 1: Create container and inject artifact ────────────────────
echo "=== Phase 1: Creating validation container ==="

NETWORK_FLAG=""
if [ "$AIRGAP" = true ]; then
    NETWORK_FLAG="--network=none"
fi

docker create \
    --name "$CONTAINER_NAME" \
    $NETWORK_FLAG \
    --memory=1g \
    --cpus=2 \
    "$CONTAINER_IMAGE" \
    sleep infinity >/dev/null

docker start "$CONTAINER_NAME" >/dev/null
echo "==> Container started: $CONTAINER_NAME"

docker cp "$TARBALL" "$CONTAINER_NAME:/tmp/lithoSpore-usb.tar.gz"
docker exec "$CONTAINER_NAME" bash -c "mkdir -p /opt/lithoSpore && cd /opt/lithoSpore && tar xzf /tmp/lithoSpore-usb.tar.gz && rm /tmp/lithoSpore-usb.tar.gz"
echo "==> Artifact extracted to /opt/lithoSpore"

docker exec "$CONTAINER_NAME" bash -c "cat /opt/lithoSpore/.biomeos-spore" 2>/dev/null && echo "" || true
docker exec "$CONTAINER_NAME" bash -c "file /opt/lithoSpore/bin/litho 2>/dev/null || echo 'file command not available'"
echo ""

# ── Phase 2: Self-test ───────────────────────────────────────────────
echo "=== Phase 2: Self-test (artifact integrity) ==="

SELFTEST_OUTPUT=$(docker exec "$CONTAINER_NAME" /opt/lithoSpore/bin/litho self-test --artifact-root /opt/lithoSpore 2>&1) || true
SELFTEST_EXIT=$?
echo "$SELFTEST_OUTPUT"
echo "  Exit code: $SELFTEST_EXIT"
echo ""

# ── Phase 3: Tier detection ─────────────────────────────────────────
echo "=== Phase 3: Tier detection ==="

TIER_OUTPUT=$(docker exec "$CONTAINER_NAME" /opt/lithoSpore/bin/litho tier --artifact-root /opt/lithoSpore 2>&1) || true
echo "$TIER_OUTPUT"
echo ""

# ── Phase 4: Full Tier 2 validation ─────────────────────────────────
echo "=== Phase 4: Full Tier 2 validation (7 modules) ==="
echo ""

VALIDATION_OUTPUT=$(docker exec "$CONTAINER_NAME" /opt/lithoSpore/bin/litho validate --artifact-root /opt/lithoSpore --json 2>&1) || true
VALIDATION_EXIT=$?

echo "$VALIDATION_OUTPUT"
echo ""

# ── Phase 5: Data integrity verification ────────────────────────────
echo "=== Phase 5: Data integrity verification (BLAKE3) ==="

VERIFY_OUTPUT=$(docker exec "$CONTAINER_NAME" /opt/lithoSpore/bin/litho verify --artifact-root /opt/lithoSpore 2>&1) || true
VERIFY_EXIT=$?

echo "$VERIFY_OUTPUT"
echo ""

# ── Phase 6: Retrieve results ───────────────────────────────────────
echo "=== Phase 6: Retrieving results ==="
mkdir -p "$RESULTS_DIR"

echo "$SELFTEST_OUTPUT" > "$RESULTS_DIR/self-test.txt"
echo "$TIER_OUTPUT" > "$RESULTS_DIR/tier.txt"
echo "$VALIDATION_OUTPUT" > "$RESULTS_DIR/validation.json"
echo "$VERIFY_OUTPUT" > "$RESULTS_DIR/verify.txt"

docker cp "$CONTAINER_NAME:/opt/lithoSpore/liveSpore.json" "$RESULTS_DIR/liveSpore.json" 2>/dev/null || true

docker exec "$CONTAINER_NAME" bash -c "uname -a; echo '---'; cat /etc/os-release | head -5; echo '---'; free -h 2>/dev/null || echo 'free not available'; echo '---'; df -h / 2>/dev/null || echo 'df not available'" > "$RESULTS_DIR/system-info.txt" 2>/dev/null || true

echo "  Results saved to: $RESULTS_DIR/"
ls -1 "$RESULTS_DIR/"
echo ""

# ── Phase 7: Deployment report ───────────────────────────────────────
echo "=== Phase 7: Deployment report ==="

REPORT="$RESULTS_DIR/deployment-report.toml"
TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)

{
    echo "# lithoSpore Deployment Validation Report"
    echo "# Generated by validate-lithoSpore-container.sh"
    echo ""
    echo "[meta]"
    echo "timestamp = \"$TIMESTAMP\""
    echo "deployment_pattern = \"container-$([ "$AIRGAP" = true ] && echo 'airgap' || echo 'networked')\""
    echo "container_image = \"$CONTAINER_IMAGE\""
    echo "container_name = \"$CONTAINER_NAME\""
    echo ""
    echo "[results]"
    echo "self_test_exit = $SELFTEST_EXIT"
    echo "validation_exit = $VALIDATION_EXIT"
    echo "verify_exit = $VERIFY_EXIT"
    echo ""
} > "$REPORT"

if command -v python3 &>/dev/null && echo "$VALIDATION_OUTPUT" | python3 -m json.tool &>/dev/null 2>&1; then
    eval "$(echo "$VALIDATION_OUTPUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
ms = d.get('modules', [])
print(f'PASSED={sum(1 for m in ms if m[\"status\"]==\"PASS\")}')
print(f'SKIPPED={sum(1 for m in ms if m[\"status\"]==\"SKIP\")}')
print(f'FAILED={sum(1 for m in ms if m[\"status\"]==\"FAIL\")}')
print(f'TIER={d.get(\"tier_reached\",\"?\")}')
print(f'TOTAL_CHECKS={sum(m.get(\"checks\",0) for m in ms)}')
print(f'CHECKS_PASSED={sum(m.get(\"checks_passed\",0) for m in ms)}')
" 2>/dev/null)" || true

    {
        echo "[validation]"
        echo "tier_reached = ${TIER:-0}"
        echo "modules_passed = ${PASSED:-0}"
        echo "modules_skipped = ${SKIPPED:-0}"
        echo "modules_failed = ${FAILED:-0}"
        echo "checks_total = ${TOTAL_CHECKS:-0}"
        echo "checks_passed = ${CHECKS_PASSED:-0}"
        echo ""
    } >> "$REPORT"

    echo "=================================================================="
    echo "  Container Validation Summary"
    echo "=================================================================="
    echo "  Image:       $CONTAINER_IMAGE"
    echo "  Airgap:      $AIRGAP"
    echo "  Tier:        ${TIER:-?}"
    echo "  Checks:      ${CHECKS_PASSED:-?}/${TOTAL_CHECKS:-?}"
    echo "  Passed:      ${PASSED:-?} modules"
    echo "  Skipped:     ${SKIPPED:-?} modules"
    echo "  Failed:      ${FAILED:-?} modules"
    echo "  Self-test:   exit $SELFTEST_EXIT"
    echo "  Validation:  exit $VALIDATION_EXIT"
    echo "  Verify:      exit $VERIFY_EXIT"
else
    echo "=================================================================="
    echo "  Container Validation Summary"
    echo "=================================================================="
    echo "  Self-test:   exit $SELFTEST_EXIT"
    echo "  Validation:  exit $VALIDATION_EXIT"
    echo "  Verify:      exit $VERIFY_EXIT"
fi

echo "  Results:     $RESULTS_DIR/"
echo "  Report:      $REPORT"
echo ""

if [ "$KEEP_CONTAINER" = true ]; then
    echo "  Container kept: docker exec -it $CONTAINER_NAME /bin/bash"
    echo "  To remove:      docker rm -f $CONTAINER_NAME"
fi

echo "=================================================================="

OVERALL_EXIT=0
[ "$SELFTEST_EXIT" -ne 0 ] && OVERALL_EXIT=1
[ "$VALIDATION_EXIT" -ne 0 ] && OVERALL_EXIT=1
exit "$OVERALL_EXIT"
