#!/usr/bin/env bash
# k80_nvidia470_mmiotrace.sh — Capture nvidia-470 driver's K80 init via mmiotrace.
#
# The proprietary driver (nvidia-470) successfully initializes GPCs on K80/GK210B
# while both Nouveau and our Rust code fail (GPCs return 0xbadf1100 PRI fault).
# This script captures the exact register write sequence used by nvidia-470 to
# ungate GPC power domains, including:
#   - PMU PGOB sequence (may use different registers than GK110's 0x0205xx)
#   - PRI ring station configuration
#   - PGRAPH engine initialization
#   - FECS/GPCCS firmware loading
#
# The captured mmiotrace is filtered to extract GPC-power-related registers
# and stored alongside the raw trace for analysis.
#
# Prerequisites:
#   - nvidia-470 .run installer or .deb packages available
#   - K80 die1 at 0000:4d:00.0 (or adjust BDF below)
#   - mmiotrace kernel support (CONFIG_MMIOTRACE=y, usually built-in)
#
# Usage:
#   sudo ./k80_nvidia470_mmiotrace.sh
#
# Output:
#   artifacts/k80_nvidia470_mmiotrace_YYYYMMDD_HHMMSS/
#     raw_trace.log        — full mmiotrace
#     pgob_filter.log      — PGOB-related registers (0x0205xx, 0x10a78c, PMC)
#     gr_filter.log        — GR engine registers (0x4xxxxx, 0x5xxxxx)
#     pri_filter.log       — PRI ring registers (0x12xxxx)
#     power_filter.log     — Power management (0x020xxx, 0x10axxx, PMC 0x200)
#     pre_registers.json   — BAR0 register snapshot BEFORE driver load
#     post_registers.json  — BAR0 register snapshot AFTER driver load
#     dmesg.log            — kernel log during capture

set -euo pipefail

K80_BDF="${K80_BDF:-0000:4d:00.0}"
ARTIFACTS="$(dirname "$0")/artifacts/k80_nvidia470_mmiotrace_$(date +%Y%m%d_%H%M%S)"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
log()  { echo -e "${CYAN}[mmio]${NC} $*"; }
ok()   { echo -e "${GREEN}[OK]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
fail() { echo -e "${RED}[FAIL]${NC} $*"; exit 1; }

[[ $EUID -eq 0 ]] || fail "Must run as root"
[[ -d "/sys/bus/pci/devices/$K80_BDF" ]] || fail "$K80_BDF not found"

mkdir -p "$ARTIFACTS"
log "Artifacts: $ARTIFACTS"
log "K80 BDF: $K80_BDF"

gpu_read() {
    python3 -c "
import mmap, struct
f = open('/sys/bus/pci/devices/$K80_BDF/resource0', 'r+b')
mm = mmap.mmap(f.fileno(), 0x1000000)
mm.seek(int('$1', 0))
v = struct.unpack('<I', mm.read(4))[0]
print(f'{v:#010x}')
mm.close(); f.close()
" 2>/dev/null || echo "0xDEADDEAD"
}

# ── Snapshot registers BEFORE nvidia-470 loads ──
log "Capturing pre-driver register snapshot..."
python3 -c "
import mmap, struct, json

bdf = '$K80_BDF'
f = open(f'/sys/bus/pci/devices/{bdf}/resource0', 'r+b')
mm = mmap.mmap(f.fileno(), 0x1000000)

def rd(reg):
    mm.seek(reg)
    return struct.unpack('<I', mm.read(4))[0]

regs = {}
# PMC / power / PRI
for name, addr in [
    ('PMC_ENABLE', 0x200), ('PMC_INTR', 0x100), ('PMC_INTR_EN', 0x140),
    ('PMU_CPUCTL', 0x10a100), ('PMU_PGOB', 0x10a78c), ('PMU_MAILBOX0', 0x10a040),
    ('PPWR_0520', 0x020520), ('PPWR_0524', 0x020524), ('PPWR_0528', 0x020528),
    ('PPWR_052C', 0x02052c), ('PPWR_0530', 0x020530), ('PPWR_06B4', 0x0206b4),
    ('THERM_CTRL1', 0x20004), ('PRI_RING_CMD', 0x12004c),
    ('PRI_RING_STATUS', 0x120058), ('PRI_NSTATIONS', 0x120070),
    ('PRI_NGPC', 0x120074), ('BOOT0', 0x0),
    ('GPC0_ID', 0x500000), ('GPC0_TPC', 0x502608), ('GPC1_ID', 0x508000),
    ('GR_HUB_0', 0x400000), ('FECS_CPUCTL', 0x409100), ('FECS_PC', 0x409030),
    ('GPCCS_CPUCTL', 0x41a100), ('PLL0_CTRL', 0x130000), ('PLL0_COEF', 0x130004),
]:
    try:
        regs[name] = f'{rd(addr):#010x}'
    except Exception:
        regs[name] = 'ERROR'

mm.close(); f.close()
with open('$ARTIFACTS/pre_registers.json', 'w') as out:
    json.dump(regs, out, indent=2)
print(json.dumps(regs, indent=2))
" 2>&1 | log "Pre-snapshot:"
log "$(cat "$ARTIFACTS/pre_registers.json" 2>/dev/null | head -5)..."

# ── Unbind current driver ──
SYSFS="/sys/bus/pci/devices/$K80_BDF"
if [[ -L "$SYSFS/driver" ]]; then
    DRV="$(basename "$(readlink "$SYSFS/driver")")"
    log "Unbinding $K80_BDF from $DRV"
    echo "$K80_BDF" > "/sys/bus/pci/drivers/$DRV/unbind"
    echo "" > "$SYSFS/driver_override"
    sleep 2
fi

# ── Enable mmiotrace ──
log "Enabling mmiotrace..."
DEBUGFS="/sys/kernel/debug/tracing"

echo mmiotrace > "$DEBUGFS/current_tracer" 2>/dev/null || fail "mmiotrace not available"
echo > "$DEBUGFS/trace"
echo 1 > "$DEBUGFS/tracing_on"
ok "mmiotrace enabled"

# ── Load nvidia-470 driver ──
# Try modprobe first, fall back to instructions
log "Loading nvidia-470 driver..."
DMESG_START=$(dmesg | wc -l)

if modprobe nvidia 2>/dev/null; then
    ok "nvidia module loaded"
    sleep 5
    # Probe the K80
    echo "$K80_BDF" > "/sys/bus/pci/drivers_probe" 2>/dev/null || true
    sleep 10

    log "Checking if nvidia claimed $K80_BDF..."
    if [[ -L "$SYSFS/driver" ]]; then
        DRV="$(basename "$(readlink "$SYSFS/driver")")"
        log "$K80_BDF driver: $DRV"
    fi

    # Run nvidia-smi to force full init (including GR)
    nvidia-smi -i "$K80_BDF" 2>&1 | head -5 | while IFS= read -r line; do log "  $line"; done || true
    sleep 5
else
    warn "nvidia module not available — install nvidia-470 first:"
    warn "  apt install nvidia-driver-470"
    warn "  OR: sh NVIDIA-Linux-x86_64-470.xx.xx.run --no-install --no-kernel-module"
    warn ""
    warn "Capturing what we have so far..."
fi

# ── Stop mmiotrace and capture ──
log "Stopping mmiotrace..."
echo 0 > "$DEBUGFS/tracing_on"
cat "$DEBUGFS/trace" > "$ARTIFACTS/raw_trace.log"
echo nop > "$DEBUGFS/current_tracer"
ok "Raw trace: $(wc -l < "$ARTIFACTS/raw_trace.log") lines"

# ── Capture dmesg ──
dmesg | tail -n +"$DMESG_START" > "$ARTIFACTS/dmesg.log"

# ── Filter trace for relevant registers ──
log "Filtering trace..."

# PGOB / power domain registers
rg -i '0205[0-9a-f]{2}\b|010a78c\b|000200\b|020004\b' \
    "$ARTIFACTS/raw_trace.log" > "$ARTIFACTS/power_filter.log" 2>/dev/null || true
ok "Power filter: $(wc -l < "$ARTIFACTS/power_filter.log") lines"

# GR engine registers (PGRAPH HUB + GPC)
rg -i '004[0-9a-f]{4}\b|005[0-3][0-9a-f]{4}\b|0041[89a-f][0-9a-f]{3}\b' \
    "$ARTIFACTS/raw_trace.log" > "$ARTIFACTS/gr_filter.log" 2>/dev/null || true
ok "GR filter: $(wc -l < "$ARTIFACTS/gr_filter.log") lines"

# PRI ring registers
rg -i '0012[0-9a-f]{4}\b|0013[0-3][0-9a-f]{3}\b' \
    "$ARTIFACTS/raw_trace.log" > "$ARTIFACTS/pri_filter.log" 2>/dev/null || true
ok "PRI filter: $(wc -l < "$ARTIFACTS/pri_filter.log") lines"

# ── Post-driver register snapshot ──
log "Capturing post-driver register snapshot..."
python3 -c "
import mmap, struct, json

bdf = '$K80_BDF'
try:
    f = open(f'/sys/bus/pci/devices/{bdf}/resource0', 'r+b')
    mm = mmap.mmap(f.fileno(), 0x1000000)
    def rd(reg):
        mm.seek(reg)
        return struct.unpack('<I', mm.read(4))[0]

    regs = {}
    for name, addr in [
        ('PMC_ENABLE', 0x200), ('PMU_CPUCTL', 0x10a100), ('PMU_PGOB', 0x10a78c),
        ('PPWR_0520', 0x020520), ('PPWR_0524', 0x020524), ('PPWR_0528', 0x020528),
        ('PPWR_052C', 0x02052c), ('PPWR_0530', 0x020530),
        ('PRI_NSTATIONS', 0x120070), ('PRI_NGPC', 0x120074),
        ('GPC0_ID', 0x500000), ('GPC0_TPC', 0x502608), ('GPC1_ID', 0x508000),
        ('GR_HUB_0', 0x400000), ('FECS_CPUCTL', 0x409100), ('FECS_PC', 0x409030),
        ('GPCCS_CPUCTL', 0x41a100),
    ]:
        try:
            regs[name] = f'{rd(addr):#010x}'
        except Exception:
            regs[name] = 'ERROR'
    mm.close(); f.close()
    with open('$ARTIFACTS/post_registers.json', 'w') as out:
        json.dump(regs, out, indent=2)
    print(json.dumps(regs, indent=2))
except Exception as e:
    print(f'Could not read registers: {e}')
" 2>&1 | while IFS= read -r line; do log "  $line"; done

# ── Clean up nvidia driver ──
if grep -q '^nvidia ' /proc/modules 2>/dev/null; then
    log "Unloading nvidia driver..."
    rmmod nvidia_uvm 2>/dev/null || true
    rmmod nvidia_drm 2>/dev/null || true
    rmmod nvidia_modeset 2>/dev/null || true
    rmmod nvidia 2>/dev/null || true
fi

echo ""
ok "════════════════════════════════════════════════════════"
ok "  mmiotrace capture complete"
ok "  Artifacts: $ARTIFACTS"
ok "  Key file: power_filter.log (PGOB + PMC sequence)"
ok "  Compare with: kernel gk110_pmu_pgob() at 0x0205xx"
ok "════════════════════════════════════════════════════════"
