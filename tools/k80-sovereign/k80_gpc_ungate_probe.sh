#!/usr/bin/env bash
# k80_gpc_ungate_probe.sh — Systematic GPC ungating probe for GK210B
#
# Tries MULTIPLE approaches to ungate K80 GPCs since the standard
# gk110_pmu_pgob() 0x0205xx power-step registers don't exist on GK210B
# (all writes generate PRIVRING faults).
#
# The approaches are tried in order. After each, we check if GPCs
# are accessible (GPC0 @ 0x500000 returns a non-fault value).
#
# Prerequisites:
#   - K80 bound to nouveau or vfio-pci with BAR0 accessible
#   - Root privileges
#
# Usage:
#   sudo ./k80_gpc_ungate_probe.sh [BDF]
#   sudo ./k80_gpc_ungate_probe.sh 0000:4d:00.0

set -euo pipefail

BDF="${1:-0000:4d:00.0}"
SYSFS="/sys/bus/pci/devices/$BDF"
BAR0="$SYSFS/resource0"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; NC='\033[0m'
log()  { echo -e "${CYAN}[probe]${NC} $*"; }
ok()   { echo -e "${GREEN}[OK]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
fail() { echo -e "${RED}[FAIL]${NC} $*"; }

[[ $EUID -eq 0 ]] || { fail "Must run as root"; exit 1; }
[[ -f "$BAR0" ]] || { fail "$BAR0 not found"; exit 1; }

gpu_read() {
    python3 -c "
import mmap, struct, sys
try:
    f = open('$BAR0', 'r+b')
    mm = mmap.mmap(f.fileno(), 0x1000000)
    mm.seek(int('$1', 0))
    v = struct.unpack('<I', mm.read(4))[0]
    print(f'{v:#010x}')
    mm.close(); f.close()
except Exception as e:
    print(f'ERROR:{e}', file=sys.stderr)
    print('0xDEADDEAD')
" 2>/dev/null
}

gpu_write() {
    python3 -c "
import mmap, struct
f = open('$BAR0', 'r+b')
mm = mmap.mmap(f.fileno(), 0x1000000)
mm.seek(int('$1', 0))
mm.write(struct.pack('<I', int('$2', 0)))
mm.close(); f.close()
" 2>/dev/null
}

gpu_mask() {
    python3 -c "
import mmap, struct
f = open('$BAR0', 'r+b')
mm = mmap.mmap(f.fileno(), 0x1000000)
mm.seek(int('$1', 0))
v = struct.unpack('<I', mm.read(4))[0]
v = (v & ~int('$2', 0)) | (int('$3', 0) & int('$2', 0))
mm.seek(int('$1', 0))
mm.write(struct.pack('<I', v))
mm.seek(int('$1', 0))
r = struct.unpack('<I', mm.read(4))[0]
print(f'{r:#010x}')
mm.close(); f.close()
" 2>/dev/null
}

check_gpcs() {
    local gpc0 gpc1 nstations gr_hub
    gpc0=$(gpu_read 0x500000)
    gpc1=$(gpu_read 0x508000)
    nstations=$(gpu_read 0x120070)
    gr_hub=$(gpu_read 0x400000)
    echo "GPC0=$gpc0 GPC1=$gpc1 NSTATIONS=$nstations GR_HUB=$gr_hub"
    if [[ "$gpc0" != "0xbadf1100" && "$gpc0" != "0xDEADDEAD" && "$gpc0" != "0x00000000" ]]; then
        return 0
    fi
    return 1
}

snapshot() {
    local label="$1"
    local pmc gpc0 nstations pmu_cpuctl fecs therm pgob
    pmc=$(gpu_read 0x200)
    gpc0=$(gpu_read 0x500000)
    nstations=$(gpu_read 0x120070)
    pmu_cpuctl=$(gpu_read 0x10a100)
    fecs=$(gpu_read 0x409100)
    therm=$(gpu_read 0x020004)
    pgob=$(gpu_read 0x10a78c)
    log "$label: PMC=$pmc GPC0=$gpc0 NSTATIONS=$nstations PMU=$pmu_cpuctl FECS=$fecs THERM=$therm PSW=$pgob"
}

pri_ring_enumerate() {
    log "  PRI ring: sending ENUM command (0x12004c=0x04)..."
    gpu_write 0x12004c 0x00000004
    sleep 0.2
    local status nstations ngpc
    status=$(gpu_read 0x120058)
    nstations=$(gpu_read 0x120070)
    ngpc=$(gpu_read 0x120074)
    log "  PRI ring: STATUS=$status NSTATIONS=$nstations NGPC=$ngpc"
}

log "╔════════════════════════════════════════════════════════╗"
log "║  K80 GPC Ungate Probe — GK210B (BDF: $BDF)   ║"
log "╚════════════════════════════════════════════════════════╝"
echo ""

BOOT0=$(gpu_read 0x0)
log "BOOT0=$BOOT0"
snapshot "BASELINE"
echo ""

# ── Check fuse bits ──
log "── FUSE CHECK ──"
FUSE_31C=$(gpu_read 0x2103c)
FUSE_SPARE=$(gpu_read 0x21c)
FUSE_GPC=$(gpu_read 0x21c00)
log "  FUSE_31C=$FUSE_31C FUSE_SPARE=$FUSE_SPARE FUSE_GPC=$FUSE_GPC"
echo ""

# ── Check THERM/PPWR register accessibility ──
log "── REGISTER ACCESSIBILITY ──"
for reg_name_addr in \
    "THERM_CTRL1:0x020004" "PPWR_0520:0x020520" "PPWR_0524:0x020524" \
    "PPWR_0528:0x020528" "PPWR_052C:0x02052c" "PPWR_0530:0x020530" \
    "PPWR_06B4:0x0206b4" "PPWR_HUB:0x020000" "PPWR_08:0x020008" \
    "PTHERM_00:0x020000" "PMC_200:0x000200" "PMC_204:0x000204" \
    "PMU_PSW:0x10a78c" "PMU_CPUCTL:0x10a100" "PMU_MBBOX0:0x10a040" \
    "PRI_CMD:0x12004c" "PRI_STAT:0x120058" "PRI_NSTAT:0x120070"
do
    IFS=: read -r name addr <<< "$reg_name_addr"
    val=$(gpu_read "$addr")
    printf "  %-14s @ %-10s = %s\n" "$name" "$addr" "$val"
done
echo ""

# ── Approach A: GK104-style THERM PGOB ──
log "── APPROACH A: GK104-style THERM PGOB (0x020004) ──"
log "  (GK104 uses NV_THERM_CTRL_1 at 0x020004 for PGOB)"
log "  Step 1: Disable PGRAPH (PMC bit 12 = 0)"
gpu_mask 0x200 0x00001000 0x00000000
sleep 0.05
log "  Step 2: Set PMC bit 27 (PGOB enable)"
gpu_mask 0x200 0x08000000 0x08000000
sleep 0.1
log "  Step 3: PSW handshake (0x10a78c)"
gpu_mask 0x10a78c 0x00000002 0x00000002
gpu_mask 0x10a78c 0x00000001 0x00000001
gpu_mask 0x10a78c 0x00000001 0x00000000
log "  Step 4: THERM_CTRL_1 = 0x40000000 (disable power gating)"
gpu_mask 0x020004 0xc0000000 0x40000000
sleep 0.1
log "  Step 5: PSW handshake #2"
gpu_mask 0x10a78c 0x00000002 0x00000000
gpu_mask 0x10a78c 0x00000001 0x00000001
gpu_mask 0x10a78c 0x00000001 0x00000000
log "  Step 6: Clear PMC bit 27, re-enable PGRAPH"
gpu_mask 0x200 0x08000000 0x00000000
gpu_mask 0x200 0x00001000 0x00001000
sleep 0.1
log "  Step 7: Re-enumerate PRI ring"
pri_ring_enumerate
log "  Checking GPCs..."
if check_gpcs; then
    ok "  APPROACH A SUCCEEDED!"
    snapshot "AFTER_A"
    exit 0
else
    fail "  Approach A: GPCs still gated"
fi
echo ""

# ── Approach B: PMC PGRAPH reset + PRI ring re-enumerate ──
log "── APPROACH B: PGRAPH Reset + PRI Ring Re-enumerate ──"
log "  (Reset PGRAPH via PMC, wait, then re-enable and re-enumerate)"
log "  Step 1: Clear PMC bit 12 (disable PGRAPH)"
gpu_mask 0x200 0x00001000 0x00000000
gpu_read 0x200 > /dev/null
log "  Step 2: Wait 100ms"
sleep 0.1
log "  Step 3: Set PMC bit 12 (re-enable PGRAPH)"
gpu_mask 0x200 0x00001000 0x00001000
gpu_read 0x200 > /dev/null
sleep 0.1
log "  Step 4: Re-enumerate PRI ring"
pri_ring_enumerate
log "  Checking GPCs..."
if check_gpcs; then
    ok "  APPROACH B SUCCEEDED!"
    snapshot "AFTER_B"
    exit 0
else
    fail "  Approach B: GPCs still gated"
fi
echo ""

# ── Approach C: PMC full engine cycle + extended PRI ring ──
log "── APPROACH C: Full PMC Engine Cycle ──"
log "  (Toggle ALL relevant PMC bits with extended delays)"
PMC_ORIG=$(gpu_read 0x200)
log "  PMC original: $PMC_ORIG"
log "  Step 1: Disable PGRAPH + PBFB + PBUS (bits 12, 8, 1)"
gpu_mask 0x200 0x00001100 0x00000000
sleep 0.2
log "  Step 2: Re-enable all"
gpu_mask 0x200 0x00001100 0x00001100
gpu_read 0x200 > /dev/null
sleep 0.5
log "  Step 3: PRI ring ACK then ENUM"
gpu_write 0x12004c 0x00000002
sleep 0.1
pri_ring_enumerate
log "  Checking GPCs..."
if check_gpcs; then
    ok "  APPROACH C SUCCEEDED!"
    snapshot "AFTER_C"
    exit 0
else
    fail "  Approach C: GPCs still gated"
fi
echo ""

# ── Approach D: PMU falcon start ──
log "── APPROACH D: Manual PMU Falcon Start ──"
PMU_CPU=$(gpu_read 0x10a100)
log "  PMU CPUCTL=$PMU_CPU"
if [[ "$PMU_CPU" == "0x00000020" || "$PMU_CPU" == "0x00000010" ]]; then
    log "  PMU is halted/stopped. Attempting to start..."
    log "  Step 1: Check if IMEM has firmware"
    PMU_IMEM0=$(gpu_read 0x10a184)
    PMU_DMEM0=$(gpu_read 0x10a1c4)
    log "  IMEM[0]=$PMU_IMEM0 DMEM[0]=$PMU_DMEM0"
    log "  Step 2: Reset PMU falcon"
    gpu_mask 0x022210 0x00000001 0x00000000
    sleep 0.05
    gpu_mask 0x022210 0x00000001 0x00000001
    gpu_read 0x022210 > /dev/null
    sleep 0.1
    log "  Step 3: Wait for scrub"
    sleep 0.2
    log "  Step 4: Set BOOTVEC=0, STARTCPU"
    gpu_write 0x10a104 0x00000000
    gpu_write 0x10a10c 0x00000000
    gpu_write 0x10a100 0x00000002
    sleep 0.5
    PMU_CPU2=$(gpu_read 0x10a100)
    PMU_PC=$(gpu_read 0x10a030)
    log "  PMU after start: CPUCTL=$PMU_CPU2 PC=$PMU_PC"
    log "  Step 5: Re-enumerate PRI ring"
    pri_ring_enumerate
    log "  Checking GPCs..."
    if check_gpcs; then
        ok "  APPROACH D SUCCEEDED!"
        snapshot "AFTER_D"
        exit 0
    else
        fail "  Approach D: GPCs still gated"
    fi
else
    log "  PMU already running (CPUCTL=$PMU_CPU), skipping"
fi
echo ""

# ── Approach E: PMC bit 27 clear without PGOB steps ──
log "── APPROACH E: Direct PMC Bit 27 Control ──"
log "  (On some GK210B, bit 27 alone controls GPC power)"
PMC_VAL=$(gpu_read 0x200)
log "  PMC=$PMC_VAL"
BIT27=$(($(printf '%d' "$PMC_VAL") & 0x08000000))
if [[ $BIT27 -ne 0 ]]; then
    log "  PMC bit 27 IS SET — clearing (disabling PGOB gate)"
    gpu_mask 0x200 0x08000000 0x00000000
    sleep 0.2
    pri_ring_enumerate
    log "  Checking GPCs..."
    if check_gpcs; then
        ok "  APPROACH E SUCCEEDED!"
        snapshot "AFTER_E"
        exit 0
    else
        fail "  Approach E: GPCs still gated"
    fi
else
    log "  PMC bit 27 already clear — trying SET then CLEAR"
    gpu_mask 0x200 0x08000000 0x08000000
    sleep 0.1
    gpu_mask 0x200 0x08000000 0x00000000
    sleep 0.2
    pri_ring_enumerate
    log "  Checking GPCs..."
    if check_gpcs; then
        ok "  APPROACH E SUCCEEDED!"
        snapshot "AFTER_E"
        exit 0
    else
        fail "  Approach E: GPCs still gated"
    fi
fi
echo ""

# ── Approach F: Extended PGOB with PMU PSW + different step values ──
log "── APPROACH F: GK210B Power Step Variants ──"
log "  (Trying different power step register patterns for GK210B)"
for variant in "disable_all" "sequential" "reverse"; do
    log "  Variant: $variant"
    gpu_mask 0x200 0x00001000 0x00000000
    gpu_read 0x200 > /dev/null
    gpu_mask 0x200 0x08000000 0x08000000
    sleep 0.05
    gpu_mask 0x10a78c 0x00000002 0x00000002
    gpu_mask 0x10a78c 0x00000001 0x00000001
    gpu_mask 0x10a78c 0x00000001 0x00000000

    case "$variant" in
        disable_all)
            gpu_write 0x020520 0x00000000
            gpu_write 0x020524 0x00000000
            gpu_write 0x020528 0x00000000
            gpu_write 0x02052c 0x00000000
            gpu_write 0x020530 0x00000000
            ;;
        sequential)
            gpu_write 0x020520 0x0000000c
            sleep 0.01
            gpu_write 0x020524 0x0000000c
            sleep 0.01
            gpu_write 0x020528 0x0000000c
            sleep 0.01
            gpu_write 0x02052c 0x0000000c
            sleep 0.01
            gpu_write 0x020530 0x0000000c
            ;;
        reverse)
            gpu_write 0x020530 0x0000000c
            sleep 0.01
            gpu_write 0x02052c 0x00000000
            sleep 0.01
            gpu_write 0x020528 0x00000000
            sleep 0.01
            gpu_write 0x020524 0x00000000
            sleep 0.01
            gpu_write 0x020520 0x00000000
            ;;
    esac
    sleep 0.05

    gpu_mask 0x10a78c 0x00000002 0x00000000
    gpu_mask 0x10a78c 0x00000001 0x00000001
    gpu_mask 0x10a78c 0x00000001 0x00000000
    gpu_mask 0x200 0x08000000 0x00000000
    gpu_mask 0x200 0x00001000 0x00001000
    gpu_read 0x200 > /dev/null
    sleep 0.2
    pri_ring_enumerate
    if check_gpcs; then
        ok "  APPROACH F ($variant) SUCCEEDED!"
        snapshot "AFTER_F_$variant"
        exit 0
    else
        fail "  Approach F ($variant): GPCs still gated"
    fi
done
echo ""

# ── Approach G: VBIOS DEVINIT re-trigger ──
log "── APPROACH G: VBIOS DEVINIT Re-trigger ──"
log "  (Trigger a PCIe FLR or secondary bus reset to re-run VBIOS POST)"
BRIDGE=$(basename "$(readlink "$SYSFS/../..")" 2>/dev/null || echo "none")
log "  PCIe bridge: $BRIDGE"
log "  Step 1: Check if FLR capability exists"
FLR_CAP=$(setpci -s "$BDF" CAP_EXP+8.l 2>/dev/null || echo "0")
log "  PCIe DevCtl: $FLR_CAP"
log "  (Not attempting FLR — would need re-bind. Logging for reference.)"
echo ""

# ── Approach H: Scan for GK210B-specific power registers ──
log "── APPROACH H: Power Register Scan ──"
log "  (Scan PPWR range 0x020000-0x020FFF for live registers)"
LIVE_COUNT=0
for offset in $(seq 0x020000 4 0x020100); do
    hex=$(printf "0x%06x" $offset)
    val=$(gpu_read "$hex")
    if [[ "$val" != "0x00000000" && "$val" != "0xbadf1100" && "$val" != "0xbadf1002" && "$val" != "0xDEADDEAD" && "$val" != "0xffffffff" ]]; then
        printf "  [LIVE] %-10s = %s\n" "$hex" "$val"
        LIVE_COUNT=$((LIVE_COUNT + 1))
    fi
done
log "  PPWR scan: $LIVE_COUNT live registers in 0x020000-0x020100"

log "  (Scan THERM range 0x020000-0x020020)"
for offset in 0x020000 0x020004 0x020008 0x02000c 0x020010 0x020014 0x020018 0x02001c 0x020020; do
    val=$(gpu_read "$offset")
    printf "  THERM %s = %s\n" "$offset" "$val"
done

log "  (Scan PMU I/O 0x10a780-0x10a7a0)"
for offset in 0x10a780 0x10a784 0x10a788 0x10a78c 0x10a790 0x10a794 0x10a798 0x10a79c 0x10a7a0; do
    val=$(gpu_read "$offset")
    printf "  PMU   %s = %s\n" "$offset" "$val"
done
echo ""

# ── Final status ──
snapshot "FINAL"
echo ""
warn "All approaches exhausted. GPCs remain power-gated."
warn "Next steps:"
warn "  1. Run nvidia-470 mmiotrace to capture proprietary driver's sequence"
warn "  2. Dump VBIOS and analyze DEVINIT tables with envytools"
warn "  3. Check if I2C/PMBus power controller exists on K80 board"
warn "  4. Try secondary bus reset (SBR) to re-trigger VBIOS POST"
exit 1
