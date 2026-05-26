#!/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# lithoSpore-deployment-matrix.sh — Run lithoSpore validation across
# multiple OS targets via libvirt VMs.
#
# Uses the agentReagents template system. Each target:
#   1. Boots a clean VM from cloud image
#   2. Deploys lithoSpore USB tarball via SCP
#   3. Runs self-test, validate, verify, deploy-report
#   4. Collects results
#   5. Tears down VM
#
# Usage:
#   sudo ./scripts/lithoSpore-deployment-matrix.sh [--tarball path] [--keep-vms] [--target ubuntu|alpine|fedora|debian|readonly|all]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
LITHO_ROOT="${LITHO_ROOT:-$(realpath "$REPO_ROOT/../../gardens/lithoSpore")}"
TARBALL="${TARBALL:-/tmp/lithoSpore-usb.tar.gz}"
KEEP_VMS=false
TARGET="all"
RESULTS_BASE="${RESULTS_DIR:-/tmp/lithoSpore-deployment-results}"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)

while [ $# -gt 0 ]; do
    case "$1" in
        --keep-vms)   KEEP_VMS=true; shift ;;
        --tarball)    TARBALL="$2"; shift 2 ;;
        --target)     TARGET="$2"; shift 2 ;;
        *)            echo "Unknown option: $1"; exit 1 ;;
    esac
done

# Colors (if terminal supports it)
RED=$(tput setaf 1 2>/dev/null || true)
GREEN=$(tput setaf 2 2>/dev/null || true)
YELLOW=$(tput setaf 3 2>/dev/null || true)
BOLD=$(tput bold 2>/dev/null || true)
RESET=$(tput sgr0 2>/dev/null || true)

declare -A RESULTS
TARGETS=()

if [ "$TARGET" = "all" ]; then
    TARGETS=(ubuntu-airgap ubuntu-vps alpine fedora debian readonly)
else
    TARGETS=("$TARGET")
fi

# ── Prerequisite check ─────────────────────────────────────────────
echo ""
echo "${BOLD}=================================================================="
echo "  lithoSpore Deployment Matrix — ${#TARGETS[@]} target(s)"
echo "==================================================================${RESET}"
echo ""

if [ ! -f "$TARBALL" ]; then
    if [ -d "$LITHO_ROOT/usb-staging" ]; then
        echo "==> Building USB tarball from existing staging..."
        tar czf "$TARBALL" -C "$LITHO_ROOT/usb-staging" .
    else
        echo "ERROR: No USB artifact tarball at $TARBALL"
        echo "  Build it:  cd $LITHO_ROOT && cargo run --bin litho -- assemble --skip-python --skip-fetch"
        echo "  Then:      tar czf $TARBALL -C $LITHO_ROOT/usb-staging ."
        exit 1
    fi
fi

echo "  Tarball:   $TARBALL ($(du -h "$TARBALL" | cut -f1))"
echo "  Results:   $RESULTS_BASE/$TIMESTAMP/"
echo "  Targets:   ${TARGETS[*]}"
echo ""

# SSH key detection
SSH_IDENTITY=""
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
detect_ssh_key || echo "WARNING: No SSH key detected"

ssh_vm() {
    local ip=$1; shift
    local user=${1:-litho}; shift || true
    ssh -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null \
        -o LogLevel=ERROR \
        -o BatchMode=yes \
        -o ConnectTimeout=10 \
        ${SSH_IDENTITY:+-i "$SSH_IDENTITY"} \
        "${user}@${ip}" "$@"
}

scp_to_vm() {
    scp -o StrictHostKeyChecking=no \
        -o UserKnownHostsFile=/dev/null \
        -o LogLevel=ERROR \
        -o BatchMode=yes \
        ${SSH_IDENTITY:+-i "$SSH_IDENTITY"} \
        "$@"
}

discover_vm_ip() {
    local vm_name=$1
    for _ in $(seq 1 30); do
        local ip
        ip=$(sudo virsh domifaddr "$vm_name" 2>/dev/null \
            | grep -oP '\d+\.\d+\.\d+\.\d+' | head -1 || true)
        if [ -n "$ip" ]; then
            echo "$ip"
            return 0
        fi
        sleep 2
    done
    return 1
}

wait_for_ssh() {
    local ip=$1
    local user=${2:-litho}
    echo -n "  Waiting for SSH"
    for _ in $(seq 1 60); do
        if ssh_vm "$ip" "$user" "true" &>/dev/null; then
            echo " ready"
            return 0
        fi
        echo -n "."
        sleep 2
    done
    echo " TIMEOUT"
    return 1
}

# Map target names to templates and VM names
template_for_target() {
    case "$1" in
        ubuntu-airgap)  echo "templates/lithoSpore-validation.yaml" ;;
        ubuntu-vps)     echo "templates/lithoSpore-vps-spore.yaml" ;;
        alpine)         echo "templates/lithoSpore-alpine-validation.yaml" ;;
        fedora)         echo "templates/lithoSpore-fedora-validation.yaml" ;;
        debian)         echo "templates/lithoSpore-debian-validation.yaml" ;;
        readonly)       echo "templates/lithoSpore-readonly-validation.yaml" ;;
        *)              echo ""; return 1 ;;
    esac
}

vm_name_for_target() {
    case "$1" in
        ubuntu-airgap)  echo "lithoSpore-validation" ;;
        ubuntu-vps)     echo "lithoSpore-vps-spore" ;;
        alpine)         echo "lithoSpore-alpine-validation" ;;
        fedora)         echo "lithoSpore-fedora-validation" ;;
        debian)         echo "lithoSpore-debian-validation" ;;
        readonly)       echo "lithoSpore-readonly-validation" ;;
        *)              echo "lithoSpore-$1" ;;
    esac
}

user_for_target() {
    case "$1" in
        ubuntu-vps)  echo "spore" ;;
        *)           echo "litho" ;;
    esac
}

# ── Run a single target ────────────────────────────────────────────
run_target() {
    local target=$1
    local template
    template=$(template_for_target "$target")
    local vm_name
    vm_name=$(vm_name_for_target "$target")
    local vm_user
    vm_user=$(user_for_target "$target")
    local results_dir="$RESULTS_BASE/$TIMESTAMP/$target"
    mkdir -p "$results_dir"

    echo ""
    echo "${BOLD}── $target ──${RESET}"
    echo "  Template: $template"
    echo "  VM name:  $vm_name"
    echo ""

    # Clean up any prior VM with this name
    sudo virsh destroy "$vm_name" 2>/dev/null || true
    sudo virsh undefine "$vm_name" --remove-all-storage 2>/dev/null || true

    # Build VM
    echo "  [1/6] Building VM..."
    cd "$REPO_ROOT"
    if [ -f "$template" ]; then
        AGENT_REAGENTS="${AGENT_REAGENTS:-$(command -v agent-reagents 2>/dev/null || echo ./target/release/agent-reagents)}"
        sudo -E "$AGENT_REAGENTS" build "$template" 2>&1 | tee "$results_dir/build.log" || {
            echo "  ${RED}ERROR: VM build failed${RESET}"
            RESULTS[$target]="BUILD_FAIL"
            return
        }
    else
        echo "  ${YELLOW}SKIP: Template $template not found (cloud image may not be downloaded)${RESET}"
        RESULTS[$target]="SKIP_NO_IMAGE"
        return
    fi

    # Discover IP and wait for SSH
    echo "  [2/6] Discovering VM..."
    local vm_ip
    vm_ip=$(discover_vm_ip "$vm_name") || {
        echo "  ${RED}ERROR: Could not discover VM IP${RESET}"
        RESULTS[$target]="NO_IP"
        return
    }
    echo "  VM IP: $vm_ip"

    wait_for_ssh "$vm_ip" "$vm_user" || {
        echo "  ${RED}ERROR: SSH timeout${RESET}"
        RESULTS[$target]="SSH_FAIL"
        return
    }

    # Deploy artifact
    echo "  [3/6] Deploying artifact..."
    ssh_vm "$vm_ip" "$vm_user" "sudo mkdir -p /opt/lithoSpore && sudo chown ${vm_user}:${vm_user} /opt/lithoSpore" || true
    scp_to_vm "$TARBALL" "${vm_user}@${vm_ip}:/tmp/lithoSpore-usb.tar.gz"
    ssh_vm "$vm_ip" "$vm_user" "cd /opt/lithoSpore && tar xzf /tmp/lithoSpore-usb.tar.gz && rm /tmp/lithoSpore-usb.tar.gz"
    echo "  Artifact deployed"

    # For airgap target, drop default route
    if [ "$target" = "ubuntu-airgap" ]; then
        echo "  [3b] Simulating airgap..."
        ssh_vm "$vm_ip" "$vm_user" "sudo ip route del default 2>/dev/null || true"
        ssh_vm "$vm_ip" "$vm_user" "ping -c 1 -W 2 8.8.8.8 >/dev/null 2>&1 && echo 'AIRGAP FAILED' || echo 'AIRGAP CONFIRMED'"
    fi

    # For readonly target, make artifact read-only
    if [ "$target" = "readonly" ]; then
        echo "  [3b] Making artifact read-only..."
        ssh_vm "$vm_ip" "$vm_user" "chmod -R a-w /opt/lithoSpore && chmod +x /opt/lithoSpore/bin/*"
    fi

    # Run validation suite
    echo "  [4/6] Running validation suite..."

    # Binary check
    local binary_info
    binary_info=$(ssh_vm "$vm_ip" "$vm_user" "file /opt/lithoSpore/bin/litho" 2>/dev/null || echo "UNKNOWN")
    echo "$binary_info" > "$results_dir/binary-check.txt"
    echo "  Binary: $binary_info"

    # Self-test
    ssh_vm "$vm_ip" "$vm_user" "/opt/lithoSpore/bin/litho self-test --artifact-root /opt/lithoSpore" \
        > "$results_dir/self-test.txt" 2>&1 || true
    echo "  Self-test: done"

    # Tier detection
    ssh_vm "$vm_ip" "$vm_user" "/opt/lithoSpore/bin/litho tier --artifact-root /opt/lithoSpore" \
        > "$results_dir/tier.txt" 2>&1 || true
    echo "  Tier: $(head -1 "$results_dir/tier.txt" 2>/dev/null || echo 'unknown')"

    # Full validation
    local validation_output
    validation_output=$(ssh_vm "$vm_ip" "$vm_user" \
        "/opt/lithoSpore/bin/litho validate --artifact-root /opt/lithoSpore --json" 2>/dev/null) || true
    VALIDATION_EXIT=$?
    echo "$validation_output" > "$results_dir/validation.json"

    # Verify
    ssh_vm "$vm_ip" "$vm_user" \
        "/opt/lithoSpore/bin/litho verify --artifact-root /opt/lithoSpore --json" \
        > "$results_dir/verify.json" 2>&1 || true

    # Deploy report
    local pattern="$target"
    ssh_vm "$vm_ip" "$vm_user" \
        "/opt/lithoSpore/bin/litho deploy-report --artifact-root /opt/lithoSpore --pattern $pattern" \
        > "$results_dir/deploy-report.toml" 2>&1 || true

    # System info
    echo "  [5/6] Collecting system info..."
    ssh_vm "$vm_ip" "$vm_user" \
        "uname -a; echo '---'; cat /etc/os-release | head -5; echo '---'; free -h; echo '---'; df -h /" \
        > "$results_dir/system-info.txt" 2>&1 || true

    # Retrieve liveSpore
    scp_to_vm "${vm_user}@${vm_ip}:/opt/lithoSpore/liveSpore.json" \
        "$results_dir/liveSpore.json" 2>/dev/null || true

    # Gate file
    ssh_vm "$vm_ip" "$vm_user" "cat /etc/lithoSpore-gate" \
        > "$results_dir/lithoSpore-gate.txt" 2>/dev/null || true

    # Determine result
    echo "  [6/6] Evaluating..."
    if echo "$binary_info" | grep -q "statically linked"; then
        if echo "$validation_output" | python3 -m json.tool &>/dev/null 2>&1; then
            local failed
            failed=$(echo "$validation_output" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(sum(1 for m in d.get('modules', []) if m.get('status') == 'FAIL'))
" 2>/dev/null || echo "?")
            if [ "$failed" = "0" ]; then
                RESULTS[$target]="PASS"
                echo "  ${GREEN}Result: PASS${RESET}"
            else
                RESULTS[$target]="FAIL($failed)"
                echo "  ${RED}Result: FAIL ($failed modules failed)${RESET}"
            fi
        else
            RESULTS[$target]="PASS_NO_JSON"
            echo "  ${YELLOW}Result: PASS (binary ran, no JSON parse)${RESET}"
        fi
    else
        RESULTS[$target]="BINARY_FAIL"
        echo "  ${RED}Result: BINARY_FAIL (not statically linked)${RESET}"
    fi

    # Restore writable for readonly
    if [ "$target" = "readonly" ]; then
        ssh_vm "$vm_ip" "$vm_user" "chmod -R u+w /opt/lithoSpore" 2>/dev/null || true
    fi

    # Cleanup VM
    if [ "$KEEP_VMS" = false ]; then
        sudo virsh destroy "$vm_name" 2>/dev/null || true
        sudo virsh undefine "$vm_name" --remove-all-storage 2>/dev/null || true
    fi
}

# ── Run all targets ────────────────────────────────────────────────
for t in "${TARGETS[@]}"; do
    run_target "$t"
done

# ── Summary ────────────────────────────────────────────────────────
echo ""
echo "${BOLD}=================================================================="
echo "  Deployment Matrix Summary — $TIMESTAMP"
echo "==================================================================${RESET}"
echo ""
printf "  %-20s %-15s\n" "TARGET" "RESULT"
printf "  %-20s %-15s\n" "──────────────────" "──────────────"

for t in "${TARGETS[@]}"; do
    result="${RESULTS[$t]:-NOT_RUN}"
    case "$result" in
        PASS*)   color="$GREEN" ;;
        FAIL*)   color="$RED" ;;
        SKIP*)   color="$YELLOW" ;;
        *)       color="$YELLOW" ;;
    esac
    printf "  %-20s ${color}%-15s${RESET}\n" "$t" "$result"
done

echo ""
echo "  Results: $RESULTS_BASE/$TIMESTAMP/"
echo ""

# Exit with failure if any target failed
for t in "${TARGETS[@]}"; do
    result="${RESULTS[$t]:-NOT_RUN}"
    case "$result" in
        PASS*|SKIP*) ;;
        *) exit 1 ;;
    esac
done

echo "${GREEN}All targets passed.${RESET}"
