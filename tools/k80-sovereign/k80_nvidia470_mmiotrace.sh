#!/usr/bin/env bash
# k80_nvidia470_mmiotrace.sh — Host-side mmiotrace of nvidia-470 initializing K80.
#
# Swaps nvidia-580 → nvidia-470 under mmiotrace, captures K80 GPC init, swaps back.
# Display goes dark for ~30 seconds during the swap.
#
# Usage: sudo ./k80_nvidia470_mmiotrace.sh
#
# Artifacts saved to: artifacts/k80_nvidia470_mmiotrace_<timestamp>/

set -euo pipefail

K80_DIE0="0000:4c:00.0"
K80_DIE1="0000:4d:00.0"
GPU_5060="0000:21:00.0"
GPU_5060_AUD="0000:21:00.1"
NV470="/var/lib/dkms/nvidia/470.256.02/$(uname -r)/x86_64/module/nvidia.ko"
TRACEDIR="/sys/kernel/tracing"
SCRIPT_DIR="$(dirname "$(readlink -f "$0")")"
ARTIFACTS="$SCRIPT_DIR/artifacts/k80_nvidia470_mmiotrace_$(date +%Y%m%d_%H%M%S)"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
log()  { echo -e "${CYAN}[mmio]${NC} $*"; }
ok()   { echo -e "${GREEN}[OK]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
fail() { echo -e "${RED}[FAIL]${NC} $*"; exit 1; }

restore_nvidia580() {
    log "Restoring nvidia-580..."
    echo 0 > "$TRACEDIR/tracing_on" 2>/dev/null || true
    echo nop > "$TRACEDIR/current_tracer" 2>/dev/null || true
    rmmod nvidia 2>/dev/null || true
    sleep 1
    modprobe nvidia 2>/dev/null || warn "Could not reload nvidia-580"
    modprobe nvidia_modeset 2>/dev/null || true
    modprobe nvidia_drm 2>/dev/null || true
    modprobe nvidia_uvm 2>/dev/null || true
    sleep 2
    systemctl start gdm 2>/dev/null || \
    systemctl start cosmic-greeter 2>/dev/null || \
    warn "Restart display manager manually"
}

[[ $EUID -eq 0 ]] || fail "Must run as root"
[[ -f "$NV470" ]] || fail "nvidia-470 module not found: $NV470"

mkdir -p "$ARTIFACTS"
log "Artifacts: $ARTIFACTS"
log "nvidia-470: $NV470"

trap restore_nvidia580 EXIT

DMESG_MARK=$(dmesg | wc -l)

# ── Phase 1: Pre-capture BAR0 snapshot ──
log "Phase 1: Cold BAR0 snapshot..."
python3 -c "
import mmap,struct,json,os
bdf='$K80_DIE0';res=f'/sys/bus/pci/devices/{bdf}/resource0'
try:
    fd=os.open(res,os.O_RDONLY|os.O_SYNC);mm=mmap.mmap(fd,0x1000000,mmap.MAP_SHARED,mmap.PROT_READ)
    def rd(r):mm.seek(r);return struct.unpack('<I',mm.read(4))[0]
    regs={n:f'{rd(a):#010x}' for n,a in [('BOOT0',0),('PMC_ENABLE',0x200),('PMU_CPUCTL',0x10a100),('PRI_NSTATIONS',0x120070),('PRI_NGPC',0x120074),('GPC0_ID',0x500000),('GR_HUB',0x400000),('FECS_CPUCTL',0x409100),('GPCCS_CPUCTL',0x41a100),('PMU_PGOB',0x10a78c),('THERM_CTRL1',0x020004),('PPWR_0520',0x020520)]}
    mm.close();os.close(fd)
    with open('$ARTIFACTS/cold_bar0.json','w') as f:json.dump(regs,f,indent=2)
    for k,v in regs.items(): print(f'  {k}: {v}')
except Exception as e:
    print(f'  Error: {e}')
" 2>&1

# ── Phase 2: Stop display manager + unload nvidia-580 ──
log "Phase 2: Stopping display manager..."
systemctl stop gdm 2>/dev/null || systemctl stop cosmic-greeter 2>/dev/null || true
sleep 3

log "Phase 2: Unloading nvidia-580..."
rmmod nvidia_uvm 2>/dev/null || true
rmmod nvidia_drm 2>/dev/null || true
rmmod nvidia_modeset 2>/dev/null || true
rmmod nvidia 2>/dev/null || true
sleep 1

if lsmod | grep -q '^nvidia '; then
    fuser -k /dev/nvidia* 2>/dev/null || true
    sleep 2
    rmmod nvidia_uvm nvidia_drm nvidia_modeset nvidia 2>/dev/null || \
        fail "Could not unload nvidia-580"
fi
ok "nvidia-580 unloaded"

# ── Phase 3: Unbind K80 from vfio-pci ──
log "Phase 3: Unbinding K80 from vfio-pci..."
for dev in "$K80_DIE0" "$K80_DIE1"; do
    if [[ -L "/sys/bus/pci/devices/$dev/driver" ]]; then
        DRV=$(basename "$(readlink "/sys/bus/pci/devices/$dev/driver")")
        echo "$dev" > "/sys/bus/pci/drivers/$DRV/unbind" 2>/dev/null || true
        echo "" > "/sys/bus/pci/devices/$dev/driver_override" 2>/dev/null || true
    fi
done
sleep 1
ok "K80 unbound"

# ── Phase 4: Enable mmiotrace ──
log "Phase 4: Enabling mmiotrace..."
echo mmiotrace > "$TRACEDIR/current_tracer"
echo > "$TRACEDIR/trace"
echo 1 > "$TRACEDIR/tracing_on"
ok "mmiotrace active"

# ── Phase 5: Load nvidia-470 ──
log "Phase 5: Loading nvidia-470..."
insmod "$NV470" 2>&1 || fail "Could not load nvidia-470"
sleep 3
ok "nvidia-470 loaded"

# ── Phase 6: Wait for K80 probe + init ──
log "Phase 6: Probing K80..."
for dev in "$K80_DIE0" "$K80_DIE1"; do
    echo "$dev" > /sys/bus/pci/drivers_probe 2>/dev/null || true
done
sleep 3

for dev in "$K80_DIE0" "$K80_DIE1"; do
    if [[ -L "/sys/bus/pci/devices/$dev/driver" ]]; then
        DRV=$(basename "$(readlink "/sys/bus/pci/devices/$dev/driver")")
        ok "$dev -> $DRV"
    else
        warn "$dev not claimed, trying explicit bind..."
        echo "$dev" > /sys/bus/pci/drivers/nvidia/bind 2>/dev/null || true
    fi
done
sleep 5

log "Phase 6: Triggering full GPU init..."
nvidia-smi 2>&1 | head -8 || warn "nvidia-smi failed"
sleep 5

# ── Phase 7: Stop mmiotrace + capture ──
log "Phase 7: Capturing mmiotrace..."
echo 0 > "$TRACEDIR/tracing_on"
cat "$TRACEDIR/trace" > "$ARTIFACTS/mmiotrace.log"
echo nop > "$TRACEDIR/current_tracer"
TRACE_LINES=$(wc -l < "$ARTIFACTS/mmiotrace.log")
ok "mmiotrace: $TRACE_LINES lines"

# ── Phase 8: Post-capture BAR0 ──
log "Phase 8: Warm BAR0 snapshot..."
python3 -c "
import mmap,struct,json,os
bdf='$K80_DIE0';res=f'/sys/bus/pci/devices/{bdf}/resource0'
try:
    fd=os.open(res,os.O_RDONLY|os.O_SYNC);mm=mmap.mmap(fd,0x1000000,mmap.MAP_SHARED,mmap.PROT_READ)
    def rd(r):mm.seek(r);return struct.unpack('<I',mm.read(4))[0]
    regs={n:f'{rd(a):#010x}' for n,a in [('BOOT0',0),('PMC_ENABLE',0x200),('PMU_CPUCTL',0x10a100),('PRI_NSTATIONS',0x120070),('PRI_NGPC',0x120074),('GPC0_ID',0x500000),('GR_HUB',0x400000),('FECS_CPUCTL',0x409100),('GPCCS_CPUCTL',0x41a100),('PMU_PGOB',0x10a78c),('THERM_CTRL1',0x020004),('PPWR_0520',0x020520)]}
    mm.close();os.close(fd)
    with open('$ARTIFACTS/warm_bar0.json','w') as f:json.dump(regs,f,indent=2)
    for k,v in regs.items(): print(f'  {k}: {v}')
except Exception as e:
    print(f'  Error: {e}')
" 2>&1

# ── Phase 9: Capture dmesg + filter trace ──
log "Phase 9: Filtering and saving..."
dmesg | tail -n +$((DMESG_MARK + 1)) > "$ARTIFACTS/dmesg.log" 2>/dev/null || dmesg > "$ARTIFACTS/dmesg.log"

grep -iE '020[0-9a-f]{3}|10a[0-9a-f]{3}' "$ARTIFACTS/mmiotrace.log" > "$ARTIFACTS/power_filter.log" 2>/dev/null || true
grep -iE '4[0-9a-f]{5}|5[0-3][0-9a-f]{4}|41[89a-f][0-9a-f]{3}' "$ARTIFACTS/mmiotrace.log" > "$ARTIFACTS/gr_filter.log" 2>/dev/null || true
grep -iE '12[0-9a-f]{4}' "$ARTIFACTS/mmiotrace.log" > "$ARTIFACTS/pri_filter.log" 2>/dev/null || true

ok "Power filter: $(wc -l < "$ARTIFACTS/power_filter.log") lines"
ok "GR filter: $(wc -l < "$ARTIFACTS/gr_filter.log") lines"
ok "PRI filter: $(wc -l < "$ARTIFACTS/pri_filter.log") lines"

# ── Phase 10: Unload nvidia-470, EXIT trap restores nvidia-580 ──
log "Phase 10: Unloading nvidia-470..."
rmmod nvidia 2>/dev/null || true

echo ""
ok "════════════════════════════════════════════════════════"
ok "  mmiotrace capture complete"
ok "  Artifacts: $ARTIFACTS"
ok "  Raw trace: $TRACE_LINES lines"
ok "════════════════════════════════════════════════════════"
