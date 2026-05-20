#!/bin/bash
# warm_handoff_titanv.sh — Warm handoff: nouveau → vfio-pci with preserved GPC state
#
# Uses a patched nouveau.ko that NOP's five teardown functions:
#   gf100_gr_fini, nvkm_pmu_fini, nvkm_mc_disable, nvkm_mc_reset, nvkm_fifo_fini
#
# This preserves GPU warm state (GPCs powered, PMC_ENABLE intact, PFIFO alive)
# across the nouveau unbind → vfio-pci bind transition, enabling sovereign
# Tier 2 compute without any vendor driver in the host path.
#
# Prerequisites:
#   - Titan V bound to vfio-pci (default boot config)
#   - No toadstool/process holding VFIO fds to the target GPU
#   - Patched nouveau.ko at artifacts/nouveau-patched.ko
#
# Usage:
#   sudo ./warm_handoff_titanv.sh [BDF]
#   Default BDF: 0000:02:00.0
#
# After success, run toadstool and test:
#   echo '{"jsonrpc":"2.0","id":1,"method":"sovereign.pmu_investigate","params":{"bdf":"BDF"}}' \
#     | nc 127.0.0.1 PORT

set -euo pipefail

BDF="${1:-0000:02:00.0}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PATCHED_KO="$SCRIPT_DIR/artifacts/nouveau-patched.ko"
ORIG_KO="/lib/modules/$(uname -r)/kernel/drivers/gpu/drm/nouveau/nouveau.ko"

log() { echo "[$(date '+%H:%M:%S')] $*"; }
fail() { log "FATAL: $*"; exit 1; }

[ "$(id -u)" -eq 0 ] || fail "Must run as root"
[ -f "$PATCHED_KO" ] || fail "Patched module not found: $PATCHED_KO"
[ -d "/sys/bus/pci/devices/$BDF" ] || fail "Device not found: $BDF"

DRIVER=$(readlink "/sys/bus/pci/devices/$BDF/driver" 2>/dev/null | xargs basename 2>/dev/null || echo "unbound")

log "Target: $BDF (currently: $DRIVER)"
log "Patched module: $PATCHED_KO"

read_reg() {
    local offset=$1
    local bar0="/sys/bus/pci/devices/$BDF/resource0"
    if [ -f "$bar0" ]; then
        python3 -c "
import mmap, struct, os
fd = os.open('$bar0', os.O_RDWR | os.O_SYNC)
m = mmap.mmap(fd, 0x20000000, access=mmap.ACCESS_READ, offset=0)
val = struct.unpack('<I', m[$offset:$offset+4])[0]
print(f'0x{val:08x}')
m.close()
os.close(fd)
" 2>/dev/null || echo "0xDEADDEAD"
    else
        echo "no-bar0"
    fi
}

log ""
log "=== Phase 1: Pre-handoff state ==="
GPC_PRE=$(read_reg 0x41A004)
CE_PRE=$(read_reg 0x104000)
PMC_PRE=$(read_reg 0x200)
log "  GPC_ENABLES = $GPC_PRE"
log "  CE0_BASE    = $CE_PRE"
log "  PMC_ENABLE  = $PMC_PRE"

log ""
log "=== Phase 2: Stop services using VFIO fd ==="
systemctl stop toadstool 2>/dev/null || true
killall -9 toadstool 2>/dev/null || true
sleep 2

log ""
log "=== Phase 3: Unbind from $DRIVER ==="
if [ "$DRIVER" = "vfio-pci" ]; then
    echo "$BDF" > "/sys/bus/pci/drivers/vfio-pci/unbind" 2>/dev/null || true
    sleep 1
elif [ "$DRIVER" = "nouveau" ]; then
    log "  Already on nouveau, will swap module"
elif [ "$DRIVER" = "unbound" ]; then
    log "  Already unbound"
fi

log ""
log "=== Phase 4: Clear driver_override ==="
echo "" > "/sys/bus/pci/devices/$BDF/driver_override"

log ""
log "=== Phase 5: Swap to patched nouveau ==="
if lsmod | grep -q '^nouveau'; then
    log "  Removing stock nouveau..."
    rmmod nouveau 2>/dev/null || true
    sleep 1
fi

log "  Loading patched nouveau (teardown NOP'd)..."
insmod "$PATCHED_KO" || fail "Failed to load patched nouveau"
sleep 1

log ""
log "=== Phase 6: Bind $BDF to patched nouveau ==="
echo "$BDF" > /sys/bus/pci/drivers/nouveau/bind || fail "Failed to bind"
log "  Waiting for GPU init (10s)..."
sleep 10

log ""
log "=== Phase 7: Verify GPU is alive (while on nouveau) ==="
GPC_WARM=$(read_reg 0x41A004)
CE_WARM=$(read_reg 0x104000)
PMC_WARM=$(read_reg 0x200)
log "  GPC_ENABLES = $GPC_WARM"
log "  CE0_BASE    = $CE_WARM"
log "  PMC_ENABLE  = $PMC_WARM"

if [[ "$GPC_WARM" == "0xbadf"* ]] || [[ "$GPC_WARM" == "0xDEAD"* ]]; then
    log "  WARNING: GPCs still appear gated while on nouveau!"
    log "  (This may be normal if nouveau uses its own BAR mapping)"
fi

log ""
log "=== Phase 8: Unbind from nouveau (teardown NOP'd) ==="
echo "$BDF" > /sys/bus/pci/drivers/nouveau/unbind
sleep 2

log ""
log "=== Phase 9: Read GPC state (the critical moment) ==="
GPC_POST=$(read_reg 0x41A004)
CE_POST=$(read_reg 0x104000)
PMC_POST=$(read_reg 0x200)
log "  GPC_ENABLES = $GPC_POST"
log "  CE0_BASE    = $CE_POST"
log "  PMC_ENABLE  = $PMC_POST"

log ""
log "=== Phase 10: Rebind to vfio-pci ==="
echo "vfio-pci" > "/sys/bus/pci/devices/$BDF/driver_override"
echo "$BDF" > /sys/bus/pci/drivers/vfio-pci/bind || fail "Failed to rebind vfio-pci"
sleep 1

log ""
log "=== Phase 11: Final state on vfio-pci ==="
GPC_FINAL=$(read_reg 0x41A004)
CE_FINAL=$(read_reg 0x104000)
PMC_FINAL=$(read_reg 0x200)
DRIVER_FINAL=$(readlink "/sys/bus/pci/devices/$BDF/driver" 2>/dev/null | xargs basename 2>/dev/null)
log "  Driver      = $DRIVER_FINAL"
log "  GPC_ENABLES = $GPC_FINAL"
log "  CE0_BASE    = $CE_FINAL"
log "  PMC_ENABLE  = $PMC_FINAL"

log ""
log "=== Phase 12: Restore stock nouveau ==="
rmmod nouveau 2>/dev/null || true
modprobe nouveau 2>/dev/null || true
log "  Stock nouveau restored"

log ""
log "========================================"
if [[ "$GPC_FINAL" != "0xbadf"* ]] && [[ "$GPC_FINAL" != "0xDEAD"* ]] && [[ "$GPC_FINAL" != "no-bar0" ]]; then
    log "  SUCCESS: GPCs ALIVE after warm handoff!"
    log "  GPC: $GPC_PRE → $GPC_WARM → $GPC_POST → $GPC_FINAL"
    log "  TIER 2 SOVEREIGN COMPUTE UNLOCKED"
else
    log "  FAILED: GPCs still gated after handoff"
    log "  GPC: $GPC_PRE → $GPC_WARM → $GPC_POST → $GPC_FINAL"
    log "  Next: investigate nvidia-470 warm handoff or kernel patch"
fi
log "========================================"
