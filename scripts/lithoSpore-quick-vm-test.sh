#!/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# lithoSpore-quick-vm-test.sh — Direct libvirt VM test without agentReagents build system.
#
# Creates a minimal VM from cloud image, injects SSH key via cloud-init,
# deploys the lithoSpore tarball, and runs the validation suite.
#
# Usage:
#   sudo ./scripts/lithoSpore-quick-vm-test.sh [--keep-vm] [--tarball path]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
TARBALL="${TARBALL:-/tmp/lithoSpore-usb.tar.gz}"
KEEP_VM=false
RESULTS_DIR="${RESULTS_DIR:-/tmp/lithoSpore-validation-results}"
VM_NAME="lithoSpore-quick-test"
VM_USER="litho"
CLOUD_IMAGE="$REPO_ROOT/images/cloud/ubuntu-24.04-server-cloudimg-amd64.img"
VM_DISK="/var/lib/libvirt/images/${VM_NAME}.qcow2"
CIDATA_ISO="/var/lib/libvirt/images/${VM_NAME}-cidata.iso"

while [ $# -gt 0 ]; do
    case "$1" in
        --keep-vm)   KEEP_VM=true; shift ;;
        --tarball)   TARBALL="$2"; shift 2 ;;
        *)           echo "Unknown option: $1"; exit 1 ;;
    esac
done

# Detect SSH key
SSH_KEY_FILE=""
SSH_PUBKEY=""
for f in /home/${SUDO_USER:-$(whoami)}/.ssh/id_ed25519 /home/${SUDO_USER:-$(whoami)}/.ssh/id_rsa; do
    if [ -f "${f}.pub" ]; then
        SSH_KEY_FILE="$f"
        SSH_PUBKEY=$(cat "${f}.pub")
        break
    fi
done

if [ -z "$SSH_PUBKEY" ]; then
    echo "ERROR: No SSH public key found"
    exit 1
fi

cleanup() {
    if [ "$KEEP_VM" = false ]; then
        echo ""
        echo "==> Cleaning up..."
        virsh destroy "$VM_NAME" 2>/dev/null || true
        virsh undefine "$VM_NAME" --remove-all-storage 2>/dev/null || true
        rm -f "$CIDATA_ISO"
    fi
}
trap cleanup EXIT

ssh_vm() {
    ssh -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null \
        -o LogLevel=ERROR \
        -o BatchMode=yes \
        -o ConnectTimeout=10 \
        -i "$SSH_KEY_FILE" \
        "${VM_USER}@${VM_IP}" "$@"
}

echo ""
echo "=================================================================="
echo "  lithoSpore Quick VM Test"
echo "=================================================================="
echo ""
echo "  Cloud image:  $CLOUD_IMAGE"
echo "  Tarball:      $TARBALL ($(du -h "$TARBALL" 2>/dev/null | cut -f1 || echo 'missing'))"
echo "  SSH key:      ${SSH_KEY_FILE}.pub"
echo ""

if [ ! -f "$TARBALL" ]; then
    echo "ERROR: Tarball not found: $TARBALL"
    exit 1
fi

if [ ! -f "$CLOUD_IMAGE" ]; then
    echo "ERROR: Cloud image not found: $CLOUD_IMAGE"
    exit 1
fi

# ── Phase 1: Create cloud-init ISO ────────────────────────────────
echo "=== Phase 1: Creating VM ==="

# Clean up any prior VM
virsh destroy "$VM_NAME" 2>/dev/null || true
virsh undefine "$VM_NAME" --remove-all-storage 2>/dev/null || true

# Create cloud-init user-data and meta-data
CIDATA_DIR=$(mktemp -d)
cat > "$CIDATA_DIR/meta-data" <<METAEOF
instance-id: litho-test-001
local-hostname: litho-test
METAEOF

cat > "$CIDATA_DIR/user-data" <<USEREOF
#cloud-config
users:
  - name: $VM_USER
    sudo: ALL=(ALL) NOPASSWD:ALL
    groups: sudo, users
    shell: /bin/bash
    lock_passwd: false
    ssh_authorized_keys:
      - $SSH_PUBKEY
packages:
  - openssh-server
  - qemu-guest-agent
runcmd:
  - systemctl enable ssh
  - mkdir -p /opt/lithoSpore
  - chown ${VM_USER}:${VM_USER} /opt/lithoSpore
  - chmod 0755 /opt/lithoSpore
USEREOF

# Create cloud-init ISO using genisoimage or mkisofs
if command -v genisoimage &>/dev/null; then
    genisoimage -output "$CIDATA_ISO" -volid cidata -joliet -rock "$CIDATA_DIR/" 2>/dev/null
elif command -v mkisofs &>/dev/null; then
    mkisofs -output "$CIDATA_ISO" -volid cidata -joliet -rock "$CIDATA_DIR/" 2>/dev/null
else
    echo "ERROR: genisoimage or mkisofs required"
    rm -rf "$CIDATA_DIR"
    exit 1
fi
rm -rf "$CIDATA_DIR"

# Copy cloud image as VM disk
cp "$CLOUD_IMAGE" "$VM_DISK"
qemu-img resize "$VM_DISK" 20G 2>/dev/null

# Create VM
virt-install \
    --name "$VM_NAME" \
    --memory 2048 \
    --vcpus 2 \
    --disk path="$VM_DISK",format=qcow2 \
    --disk path="$CIDATA_ISO",device=cdrom \
    --os-variant ubuntu24.04 \
    --network network=default \
    --graphics none \
    --noautoconsole \
    --import \
    2>&1 || {
    echo "ERROR: virt-install failed"
    exit 1
}

echo "  VM created, waiting for boot..."

# Wait for IP
VM_IP=""
for _ in $(seq 1 60); do
    VM_IP=$(virsh domifaddr "$VM_NAME" 2>/dev/null \
        | grep -oP '\d+\.\d+\.\d+\.\d+' | head -1 || true)
    [ -n "$VM_IP" ] && break
    sleep 2
done

if [ -z "$VM_IP" ]; then
    echo "ERROR: Could not discover VM IP"
    exit 1
fi
echo "  VM IP: $VM_IP"

# Wait for SSH
echo -n "  Waiting for SSH"
for _ in $(seq 1 90); do
    if ssh_vm "true" &>/dev/null; then
        echo " ready"
        break
    fi
    echo -n "."
    sleep 2
done

# Wait for cloud-init to finish
echo "  Waiting for cloud-init..."
ssh_vm "cloud-init status --wait" 2>/dev/null || sleep 15

# ── Phase 2: Deploy and test ──────────────────────────────────────
echo ""
echo "=== Phase 2: Deploying artifact ==="

scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
    -i "$SSH_KEY_FILE" \
    "$TARBALL" "${VM_USER}@${VM_IP}:/tmp/lithoSpore-usb.tar.gz"

ssh_vm "cd /opt/lithoSpore && tar xzf /tmp/lithoSpore-usb.tar.gz && rm /tmp/lithoSpore-usb.tar.gz"
echo "  Artifact deployed"

# Check binary
BINARY_INFO=$(ssh_vm "file /opt/lithoSpore/bin/litho" 2>/dev/null || echo "UNKNOWN")
echo "  Binary: $BINARY_INFO"

echo ""
echo "=== Phase 3: Simulating airgap ==="
ssh_vm "ping -c 1 -W 2 8.8.8.8 >/dev/null 2>&1 && echo 'Pre-airgap: ONLINE' || echo 'Already offline'"
ssh_vm "sudo ip route del default 2>/dev/null || true"
ssh_vm "ping -c 1 -W 2 8.8.8.8 >/dev/null 2>&1 && echo 'AIRGAP FAILED' || echo 'AIRGAP CONFIRMED'"

echo ""
echo "=== Phase 4: Running validation suite ==="

mkdir -p "$RESULTS_DIR"

# Self-test
echo "  [1/5] Self-test..."
ssh_vm "/opt/lithoSpore/bin/litho self-test --artifact-root /opt/lithoSpore" \
    > "$RESULTS_DIR/self-test.txt" 2>&1 || true
tail -3 "$RESULTS_DIR/self-test.txt"

# Tier
echo "  [2/5] Tier detection..."
ssh_vm "/opt/lithoSpore/bin/litho tier --artifact-root /opt/lithoSpore" \
    > "$RESULTS_DIR/tier.txt" 2>&1 || true
cat "$RESULTS_DIR/tier.txt"

# Validate
echo "  [3/5] Full validation..."
ssh_vm "/opt/lithoSpore/bin/litho validate --artifact-root /opt/lithoSpore --json" \
    > "$RESULTS_DIR/validation.json" 2>/dev/null || true
VALIDATION_EXIT=$?

# Verify
echo "  [4/5] Data verification..."
ssh_vm "/opt/lithoSpore/bin/litho verify --artifact-root /opt/lithoSpore --json" \
    > "$RESULTS_DIR/verify.json" 2>&1 || true

# Deploy report
echo "  [5/5] Deploy report..."
ssh_vm "/opt/lithoSpore/bin/litho deploy-report --artifact-root /opt/lithoSpore --pattern vm-airgap" \
    > "$RESULTS_DIR/deploy-report.toml" 2>&1 || true

# System info
ssh_vm "uname -a; echo '---'; cat /etc/os-release | head -5; echo '---'; free -h; echo '---'; df -h /" \
    > "$RESULTS_DIR/system-info.txt" 2>/dev/null || true

# Retrieve liveSpore
scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
    -i "$SSH_KEY_FILE" \
    "${VM_USER}@${VM_IP}:/opt/lithoSpore/liveSpore.json" \
    "$RESULTS_DIR/liveSpore.json" 2>/dev/null || true

echo ""
echo "=== Phase 5: Results ==="
echo "  Results saved to: $RESULTS_DIR/"
ls -1 "$RESULTS_DIR/"

# Parse validation JSON
echo ""
if [ -f "$RESULTS_DIR/validation.json" ] && python3 -m json.tool "$RESULTS_DIR/validation.json" &>/dev/null 2>&1; then
    eval "$(python3 -c "
import sys, json
d = json.load(open('$RESULTS_DIR/validation.json'))
ms = d.get('modules', [])
print(f'PASSED={sum(1 for m in ms if m[\"status\"]==\"PASS\")}')
print(f'SKIPPED={sum(1 for m in ms if m[\"status\"]==\"SKIP\")}')
print(f'FAILED={sum(1 for m in ms if m[\"status\"]==\"FAIL\")}')
print(f'TIER={d.get(\"tier_reached\",\"?\")}')
print(f'CHECKS={sum(m.get(\"checks_passed\",0) for m in ms)}/{sum(m.get(\"checks\",0) for m in ms)}')
" 2>/dev/null)"

    echo "=================================================================="
    echo "  Validation Summary"
    echo "=================================================================="
    echo "  VM:        $VM_NAME @ $VM_IP (airgapped)"
    echo "  OS:        Ubuntu 24.04"
    echo "  Binary:    musl-static-pie"
    echo "  Tier:      ${TIER:-?}"
    echo "  Checks:    ${CHECKS:-?}"
    echo "  Passed:    ${PASSED:-?} modules"
    echo "  Skipped:   ${SKIPPED:-?} modules"
    echo "  Failed:    ${FAILED:-?} modules"
    echo "=================================================================="
else
    echo "  Validation JSON not parseable"
    echo "  Exit code: $VALIDATION_EXIT"
    [ -f "$RESULTS_DIR/validation.json" ] && head -20 "$RESULTS_DIR/validation.json"
fi

if [ "$KEEP_VM" = true ]; then
    echo ""
    echo "  VM kept alive:  ssh -i $SSH_KEY_FILE ${VM_USER}@${VM_IP}"
fi
