#!/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# validate-lithoSpore-vps.sh — VPS "spore drop" lithoSpore validation.
#
# Simulates deploying lithoSpore to a remote VPS (no biomeOS, no primals).
# Unlike the airgapped validation, the network stays UP throughout.
# This validates the "spore drop" deployment pattern: lithoSpore delivered
# to any arbitrary Linux host via SCP, validated with network available.
#
# Phases:
#   0. Prerequisite check (tarball, SSH key, cloud image)
#   1. Build VPS-like VM via agentReagents
#   2. Deploy lithoSpore USB artifact via SCP
#   3. Run litho validate (Tier 2, network available)
#   4. Run litho verify (data integrity + upstream probe)
#   5. Retrieve results (liveSpore.json, verify output)
#   6. Compare against airgapped baseline if available
#   7. Clean up (unless --keep-vm)
#
# Usage:
#   sudo ./scripts/validate-lithoSpore-vps.sh [--keep-vm] [--tarball path]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
LITHO_ROOT="${LITHO_ROOT:-$(realpath "$REPO_ROOT/../../gardens/lithoSpore")}"
TARBALL="${TARBALL:-/tmp/lithoSpore-usb.tar.gz}"
KEEP_VM=false
RESULTS_DIR="${RESULTS_DIR:-/tmp/lithoSpore-vps-results}"
VM_USER="spore"
VM_NAME="lithoSpore-vps-spore"
VM_IP=""
SSH_IDENTITY=""

while [ $# -gt 0 ]; do
    case "$1" in
        --keep-vm)   KEEP_VM=true; shift ;;
        --tarball)   TARBALL="$2"; shift 2 ;;
        *)           echo "Unknown option: $1"; exit 1 ;;
    esac
done

cleanup() {
    if [ "$KEEP_VM" = false ] && [ -n "$VM_NAME" ]; then
        echo ""
        echo "==> Cleaning up VM: $VM_NAME"
        sudo virsh destroy "$VM_NAME" 2>/dev/null || true
        sudo virsh undefine "$VM_NAME" --remove-all-storage 2>/dev/null || true
    fi
}
trap cleanup EXIT

ssh_vm() {
    ssh -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null \
        -o LogLevel=ERROR \
        -o BatchMode=yes \
        -o ConnectTimeout=10 \
        ${SSH_IDENTITY:+-i "$SSH_IDENTITY"} \
        "${VM_USER}@${VM_IP}" "$@"
}

scp_to_vm() {
    scp -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null \
        -o LogLevel=ERROR \
        -o BatchMode=yes \
        ${SSH_IDENTITY:+-i "$SSH_IDENTITY"} \
        "$@"
}

scp_from_vm() {
    scp -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null \
        -o LogLevel=ERROR \
        -o BatchMode=yes \
        ${SSH_IDENTITY:+-i "$SSH_IDENTITY"} \
        "${VM_USER}@${VM_IP}:$1" "$2"
}

detect_ssh_key() {
    local user="${SUDO_USER:-$(whoami)}"
    local home="/home/$user"
    for key in id_ed25519 id_rsa id_ecdsa; do
        if [ -f "$home/.ssh/$key" ]; then
            SSH_IDENTITY="$home/.ssh/$key"
            return 0
        fi
    done
    return 1
}

discover_vm_ip() {
    for _ in $(seq 1 30); do
        VM_IP=$(sudo virsh domifaddr "$VM_NAME" 2>/dev/null \
            | grep -oP '\d+\.\d+\.\d+\.\d+' | head -1 || true)
        [ -n "$VM_IP" ] && return 0
        sleep 2
    done
    return 1
}

wait_for_ssh() {
    echo -n "==> Waiting for SSH"
    for i in $(seq 1 60); do
        if ssh_vm "true" &>/dev/null; then
            echo " ready (attempt $i)"
            return 0
        fi
        echo -n "."
        sleep 2
    done
    echo " TIMEOUT"
    return 1
}

echo ""
echo "=================================================================="
echo "  lithoSpore VPS Spore-Drop Validation"
echo "  Deployment pattern: remote VPS (network available)"
echo "=================================================================="
echo ""

# ── Phase 0: Prerequisites ──────────────────────────────────────────
echo "=== Phase 0: Prerequisites ==="

if [ ! -f "$TARBALL" ]; then
    if [ -d "$LITHO_ROOT/usb-staging" ]; then
        echo "==> Building USB tarball..."
        tar czf "$TARBALL" -C "$LITHO_ROOT/usb-staging" .
    else
        echo "ERROR: No USB artifact. Run:"
        echo "  cd $LITHO_ROOT && ./scripts/assemble-usb.sh --skip-python --skip-fetch"
        exit 1
    fi
fi

detect_ssh_key || echo "WARNING: No SSH key detected"

echo "  Tarball:  $TARBALL ($(du -h "$TARBALL" | cut -f1))"
echo "  SSH key:  ${SSH_IDENTITY:-none}"
echo "  Pattern:  VPS spore-drop (network UP)"
echo ""

# ── Phase 1: Build VM ───────────────────────────────────────────────
echo "=== Phase 1: Building VPS VM ==="

cd "$REPO_ROOT"
TEMPLATE="templates/lithoSpore-vps-spore.yaml"

BUILD_LOG="/tmp/lithoSpore-vps-build.log"
sudo -E ./target/release/agent-reagents build "$TEMPLATE" 2>&1 | tee "$BUILD_LOG" || {
    echo ""
    echo "ERROR: VM build failed. Checking if VM survived..."
    if sudo virsh domstate "$VM_NAME" 2>/dev/null | grep -q running; then
        echo "==> VM is running despite build error. Continuing..."
    else
        echo "==> VM not running. Cannot continue."
        exit 1
    fi
}

echo ""

echo "==> Discovering VM IP..."
if ! discover_vm_ip; then
    echo "ERROR: Could not find VM IP"
    exit 1
fi
echo "  VM: $VM_NAME @ $VM_IP"

if ! wait_for_ssh; then
    echo "ERROR: SSH not available"
    exit 1
fi

# Verify network is available (key difference from airgapped)
echo "==> Verifying network connectivity..."
ssh_vm "ping -c 1 -W 3 8.8.8.8 >/dev/null 2>&1 && echo 'Network: ONLINE' || echo 'Network: OFFLINE (unexpected!)'"
echo ""

# ── Phase 2: Deploy artifact ────────────────────────────────────────
echo "=== Phase 2: Deploying lithoSpore artifact ==="

ssh_vm "sudo mkdir -p /opt/lithoSpore && sudo chown ${VM_USER}:${VM_USER} /opt/lithoSpore"
echo "==> /opt/lithoSpore created"

scp_to_vm "$TARBALL" "${VM_USER}@${VM_IP}:/tmp/lithoSpore-usb.tar.gz"
echo "==> Tarball copied"

ssh_vm "cd /opt/lithoSpore && tar xzf /tmp/lithoSpore-usb.tar.gz && rm /tmp/lithoSpore-usb.tar.gz"
echo "==> Artifact extracted"

ssh_vm "file /opt/lithoSpore/bin/litho && ls /opt/lithoSpore/artifact/data/ | wc -l | xargs echo 'Data bundles:'"
echo ""

# ── Phase 3: Validation (network available) ─────────────────────────
echo "=== Phase 3: lithoSpore validation (NETWORK UP, Tier 2) ==="
echo ""

VALIDATION_OUTPUT=$(ssh_vm "cd /opt/lithoSpore && ./bin/litho validate --artifact-root . --json 2>&1") || true
VALIDATION_EXIT=$?

echo "$VALIDATION_OUTPUT"
echo ""

# ── Phase 4: Data Verification (upstream probe) ─────────────────────
echo "=== Phase 4: lithoSpore data verification (upstream probe) ==="
echo ""

VERIFY_OUTPUT=$(ssh_vm "cd /opt/lithoSpore && ./bin/litho verify --artifact-root . --json 2>&1") || true

echo "$VERIFY_OUTPUT"
echo ""

# ── Phase 5: Retrieve results ───────────────────────────────────────
echo "=== Phase 5: Retrieving results ==="
mkdir -p "$RESULTS_DIR"

echo "$VALIDATION_OUTPUT" > "$RESULTS_DIR/validation.json"
echo "$VERIFY_OUTPUT" > "$RESULTS_DIR/verify.json"
scp_from_vm "/opt/lithoSpore/liveSpore.json" "$RESULTS_DIR/liveSpore.json" 2>/dev/null || true
ssh_vm "cat /etc/lithoSpore-gate" > "$RESULTS_DIR/lithoSpore-gate.txt" 2>/dev/null || true
ssh_vm "uname -a; echo '---'; cat /etc/os-release | head -5; echo '---'; free -h; echo '---'; df -h /; echo '---'; ip route" > "$RESULTS_DIR/system-info.txt" 2>/dev/null || true
ssh_vm "cat /opt/lithoSpore/data_manifest.toml" > "$RESULTS_DIR/data_manifest.toml" 2>/dev/null || true

echo "  Results saved to: $RESULTS_DIR"
ls -1 "$RESULTS_DIR/"
echo ""

# ── Phase 6: Compare with airgapped baseline ─────────────────────
echo "=== Phase 6: Cross-pattern comparison ==="
AIRGAP_DIR="/tmp/lithoSpore-validation-results"

if [ -f "$AIRGAP_DIR/validation.json" ]; then
    echo "  Airgapped baseline found at $AIRGAP_DIR"

    AIRGAP_MODULES=""
    VPS_MODULES=""
    if command -v python3 &>/dev/null; then
        AIRGAP_MODULES=$(python3 -c "
import json, sys
try:
    d = json.load(open('$AIRGAP_DIR/validation.json'))
    ms = d.get('modules',[])
    print(f'{sum(1 for m in ms if m[\"status\"]==\"PASS\")}/{len(ms)} pass')
except: print('parse error')
" 2>/dev/null || echo "?")

        VPS_MODULES=$(echo "$VALIDATION_OUTPUT" | python3 -c "
import json, sys
try:
    d = json.load(sys.stdin)
    ms = d.get('modules',[])
    print(f'{sum(1 for m in ms if m[\"status\"]==\"PASS\")}/{len(ms)} pass')
except: print('parse error')
" 2>/dev/null || echo "?")
    fi

    echo "  Airgapped: $AIRGAP_MODULES"
    echo "  VPS:       $VPS_MODULES"
else
    echo "  No airgapped baseline available for comparison"
    echo "  Run validate-lithoSpore.sh first to generate one"
fi
echo ""

# ── Phase 7: Summary ────────────────────────────────────────────────
echo "=================================================================="
echo "  VPS Spore-Drop Validation Summary"
echo "=================================================================="

if command -v python3 &>/dev/null && echo "$VALIDATION_OUTPUT" | python3 -m json.tool &>/dev/null 2>&1; then
    eval "$(echo "$VALIDATION_OUTPUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
ms = d.get('modules', [])
print(f'PASSED={sum(1 for m in ms if m[\"status\"]==\"PASS\")}')
print(f'SKIPPED={sum(1 for m in ms if m[\"status\"]==\"SKIP\")}')
print(f'FAILED={sum(1 for m in ms if m[\"status\"]==\"FAIL\")}')
print(f'TIER={d.get(\"tier_reached\",\"?\")}')
print(f'CHECKS={sum(m.get(\"checks_passed\",0) for m in ms)}/{sum(m.get(\"checks\",0) for m in ms)}')
" 2>/dev/null)"

    echo "  VM:             $VM_NAME @ $VM_IP"
    echo "  OS:             Ubuntu 24.04 (network available)"
    echo "  Pattern:        VPS spore-drop"
    echo "  Tier:           $TIER"
    echo "  Checks:         $CHECKS"
    echo "  Passed:         $PASSED modules"
    echo "  Skipped:        $SKIPPED modules"
    echo "  Failed:         $FAILED modules"
    echo "  Validation:     exit $VALIDATION_EXIT"
else
    echo "  Validation exit: $VALIDATION_EXIT"
fi

# Verify results summary
if command -v python3 &>/dev/null && echo "$VERIFY_OUTPUT" | python3 -m json.tool &>/dev/null 2>&1; then
    eval "$(echo "$VERIFY_OUTPUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
s = d.get('summary', {})
print(f'VERIFY_LOCAL={s.get(\"local_files_ok\",\"?\")}/{s.get(\"local_files_total\",\"?\")}')
print(f'VERIFY_UPSTREAM={s.get(\"upstream_reachable\",\"?\")}/{s.get(\"upstream_total\",\"?\")}')
print(f'VERIFY_ONLINE={s.get(\"online\",\"?\")}')
" 2>/dev/null)"

    echo "  Data integrity: $VERIFY_LOCAL files ok"
    echo "  Upstream:       $VERIFY_UPSTREAM sources reachable (online=$VERIFY_ONLINE)"
fi

echo "  Results:        $RESULTS_DIR/"
echo ""

if [ "$KEEP_VM" = true ]; then
    echo "  VM kept alive:  ssh ${VM_USER}@${VM_IP}"
    echo "  To tear down:   sudo virsh destroy $VM_NAME && sudo virsh undefine $VM_NAME --remove-all-storage"
fi

echo "=================================================================="
exit "$VALIDATION_EXIT"
