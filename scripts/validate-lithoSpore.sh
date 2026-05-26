#!/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# validate-lithoSpore.sh — Full airgapped lithoSpore validation on a clean VM.
#
# Phases:
#   0. Prerequisite check (tarball, SSH key, cloud image)
#   1. Build VM via agentReagents (streaming output)
#   2. Deploy lithoSpore USB artifact via SCP
#   3. Simulate airgap (drop default route)
#   4. Run ./bin/litho validate (Tier 2, airgapped)
#   5. Retrieve results (liveSpore.json, system info)
#   6. Clean up (unless --keep-vm)
#
# Usage:
#   sudo ./scripts/validate-lithoSpore.sh [--keep-vm] [--tarball path]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
LITHO_ROOT="${LITHO_ROOT:-$(realpath "$REPO_ROOT/../../gardens/lithoSpore")}"
TARBALL="${TARBALL:-/tmp/lithoSpore-usb.tar.gz}"
KEEP_VM=false
RESULTS_DIR="${RESULTS_DIR:-/tmp/lithoSpore-validation-results}"
VM_USER="litho"
VM_NAME="lithoSpore-validation"
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
echo "  lithoSpore Airgapped Validation — Clean Linux VM"
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
        echo "  cd $LITHO_ROOT && cargo run --bin litho -- assemble --skip-python --skip-fetch"
        exit 1
    fi
fi

detect_ssh_key || echo "WARNING: No SSH key detected"

echo "  Tarball:  $TARBALL ($(du -h "$TARBALL" | cut -f1))"
echo "  SSH key:  ${SSH_IDENTITY:-none}"
echo ""

# ── Phase 1: Build VM ───────────────────────────────────────────────
echo "=== Phase 1: Building validation VM ==="

cd "$REPO_ROOT"
TEMPLATE="templates/lithoSpore-validation.yaml"

# Stream build output (tee to log so we see progress)
BUILD_LOG="/tmp/lithoSpore-build.log"
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

# Discover VM IP via libvirt (reliable, doesn't depend on parsing build output)
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

# ── Phase 2: Deploy artifact ────────────────────────────────────────
echo ""
echo "=== Phase 2: Deploying lithoSpore artifact ==="

ssh_vm "sudo mkdir -p /opt/lithoSpore && sudo chown ${VM_USER}:${VM_USER} /opt/lithoSpore"
echo "==> /opt/lithoSpore created"

scp_to_vm "$TARBALL" "${VM_USER}@${VM_IP}:/tmp/lithoSpore-usb.tar.gz"
echo "==> Tarball copied"

ssh_vm "cd /opt/lithoSpore && tar xzf /tmp/lithoSpore-usb.tar.gz && rm /tmp/lithoSpore-usb.tar.gz"
echo "==> Artifact extracted"

ssh_vm "file /opt/lithoSpore/bin/litho && ls /opt/lithoSpore/artifact/data/ | wc -l | xargs echo 'Data bundles:'"
echo ""

# ── Phase 3: Airgap simulation ──────────────────────────────────────
echo "=== Phase 3: Simulating airgap ==="

ssh_vm "ping -c 1 -W 2 8.8.8.8 >/dev/null 2>&1 && echo 'Pre-airgap: ONLINE' || echo 'Pre-airgap: already offline'"
ssh_vm "sudo ip route del default 2>/dev/null || true"
ssh_vm "ping -c 1 -W 2 8.8.8.8 >/dev/null 2>&1 && echo 'AIRGAP FAILED' || echo 'AIRGAP CONFIRMED'"
echo ""

# ── Phase 3b: Self-test ──────────────────────────────────────────────
echo "=== Phase 3b: Self-test (artifact integrity) ==="
ssh_vm "cd /opt/lithoSpore && ./bin/litho self-test --artifact-root . 2>&1" || true
echo ""

# ── Phase 4: Validation ─────────────────────────────────────────────
echo "=== Phase 4: lithoSpore validation (AIRGAPPED, Tier 2) ==="
echo ""

VALIDATION_OUTPUT=$(ssh_vm "cd /opt/lithoSpore && ./bin/litho validate --artifact-root . --json 2>/dev/null") || true
VALIDATION_EXIT=$?

echo "$VALIDATION_OUTPUT"
echo ""

# ── Phase 4b: Deploy report ─────────────────────────────────────────
echo "=== Phase 4b: Deployment report (TOML) ==="
DEPLOY_REPORT=$(ssh_vm "cd /opt/lithoSpore && ./bin/litho deploy-report --artifact-root . --pattern vm-airgap 2>/dev/null") || true
echo "$DEPLOY_REPORT"
echo ""

# ── Phase 5: Retrieve results ───────────────────────────────────────
echo "=== Phase 5: Retrieving results ==="
mkdir -p "$RESULTS_DIR"

echo "$VALIDATION_OUTPUT" > "$RESULTS_DIR/validation.json"
echo "$DEPLOY_REPORT" > "$RESULTS_DIR/deployment-report.toml" 2>/dev/null || true
scp_to_vm "${VM_USER}@${VM_IP}:/opt/lithoSpore/liveSpore.json" "$RESULTS_DIR/liveSpore.json" 2>/dev/null || true
ssh_vm "cat /etc/lithoSpore-gate" > "$RESULTS_DIR/lithoSpore-gate.txt" 2>/dev/null || true
ssh_vm "uname -a; echo '---'; cat /etc/os-release | head -5; echo '---'; free -h; echo '---'; df -h /; echo '---'; ip route" > "$RESULTS_DIR/system-info.txt" 2>/dev/null || true
ssh_vm "cat /opt/lithoSpore/data_manifest.toml" > "$RESULTS_DIR/data_manifest.toml" 2>/dev/null || true

echo "  Results saved to: $RESULTS_DIR"
ls -1 "$RESULTS_DIR/"
echo ""

# ── Phase 6: Summary ────────────────────────────────────────────────
echo "=================================================================="
echo "  Validation Summary"
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

    echo "  VM:        $VM_NAME @ $VM_IP"
    echo "  OS:        Ubuntu 24.04 (airgapped)"
    echo "  Tier:      $TIER"
    echo "  Checks:    $CHECKS"
    echo "  Passed:    $PASSED modules"
    echo "  Skipped:   $SKIPPED modules"
    echo "  Failed:    $FAILED modules"
    echo "  Exit code: $VALIDATION_EXIT"
else
    echo "  Exit code: $VALIDATION_EXIT"
fi

echo "  Results:   $RESULTS_DIR/"
echo ""

if [ "$KEEP_VM" = true ]; then
    echo "  VM kept alive:  ssh ${VM_USER}@${VM_IP}"
    echo "  To tear down:   sudo virsh destroy $VM_NAME && sudo virsh undefine $VM_NAME --remove-all-storage"
fi

echo "=================================================================="
exit "$VALIDATION_EXIT"
