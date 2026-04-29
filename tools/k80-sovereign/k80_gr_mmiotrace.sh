#!/usr/bin/env bash
# k80_gr_mmiotrace.sh — Capture mmiotrace of K80 GR init (FECS/GPCCS boot) via nouveau.
#
# Captures the full register-level trace of nouveau's PGRAPH init on K80
# (GK210B). Filters for FECS/GPCCS falcon registers, PGRAPH MMIO init,
# clock gating, and method dispatch registers. The resulting trace reveals
# the exact sequence coral-driver must replicate for sovereign Falcon boot.
#
# Usage:
#   sudo ./k80_gr_mmiotrace.sh [die0|die1]
#   sudo TRACE_DIR=/tmp/traces ./k80_gr_mmiotrace.sh die1

set -euo pipefail

MODE="${1:-die1}"
K80_DIE0="0000:4c:00.0"
K80_DIE1="0000:4d:00.0"
TRACE_DIR="${TRACE_DIR:-/tmp/coralreef_traces}"
TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
SETTLE_SECS="${SETTLE_SECS:-15}"

case "$MODE" in
    die0) BDF="$K80_DIE0" ;;
    die1) BDF="$K80_DIE1" ;;
    *)    echo "Usage: $0 [die0|die1]"; exit 1 ;;
esac

TRACE_FILE="$TRACE_DIR/k80_gr_${TIMESTAMP}.mmiotrace"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
log()  { echo -e "${CYAN}[k80-mmio]${NC} $*"; }
ok()   { echo -e "${GREEN}[OK]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
fail() { echo -e "${RED}[FAIL]${NC} $*"; exit 1; }

[[ $EUID -eq 0 ]] || fail "Must run as root"
[[ -d "/sys/bus/pci/devices/$BDF" ]] || fail "$BDF not found on PCI bus"

mkdir -p "$TRACE_DIR"

# ── Phase 1: Preflight ──
log "Phase 1: Preflight — K80 $MODE at $BDF"

SYSFS="/sys/bus/pci/devices/$BDF"
DRV="(none)"
[[ -L "$SYSFS/driver" ]] && DRV="$(basename "$(readlink "$SYSFS/driver")")"
log "Current driver: $DRV"

# Unbind current driver
if [[ -L "$SYSFS/driver" ]]; then
    log "Unbinding $BDF from $DRV..."
    echo "$BDF" > "/sys/bus/pci/drivers/$DRV/unbind" 2>/dev/null || true
    sleep 1
fi

# Unload nouveau if loaded
if grep -q "^nouveau " /proc/modules 2>/dev/null; then
    log "Unloading existing nouveau..."
    rmmod nouveau 2>/dev/null || warn "rmmod nouveau failed"
    sleep 1
fi

# Clear driver_override
echo "" > "$SYSFS/driver_override" 2>/dev/null || true

# ── Phase 2: Enable mmiotrace ──
log "Phase 2: Enabling mmiotrace"

TRACING="/sys/kernel/debug/tracing"
if [[ ! -d "$TRACING" ]]; then
    mount -t debugfs debugfs /sys/kernel/debug 2>/dev/null || true
fi
[[ -d "$TRACING" ]] || fail "debugfs/tracing not available"

PREV_TRACER="$(cat "$TRACING/current_tracer" 2>/dev/null || echo nop)"

echo mmiotrace > "$TRACING/current_tracer"
echo 1 > "$TRACING/tracing_on"
ok "mmiotrace enabled"

# ── Phase 3: Load nouveau (GR init + FECS/GPCCS boot captured) ──
log "Phase 3: Loading nouveau — PGRAPH/GR init will be captured"

modprobe --ignore-install nouveau 2>&1 || warn "modprobe nouveau returned error"
sleep 2

# Probe the device if nouveau didn't auto-claim it
if [[ ! -L "$SYSFS/driver" ]]; then
    echo "$BDF" > /sys/bus/pci/drivers_probe 2>/dev/null || true
fi

log "Waiting ${SETTLE_SECS}s for nouveau init to complete..."
sleep "$SETTLE_SECS"

DRV_NOW="(none)"
[[ -L "$SYSFS/driver" ]] && DRV_NOW="$(basename "$(readlink "$SYSFS/driver")")"
log "Driver after probe: $DRV_NOW"

# ── Phase 4: Capture and stop trace ──
log "Phase 4: Capturing mmiotrace"

echo 0 > "$TRACING/tracing_on"
cp "$TRACING/trace" "$TRACE_FILE"
echo "$PREV_TRACER" > "$TRACING/current_tracer"

TRACE_LINES=$(wc -l < "$TRACE_FILE")
ok "Trace captured: $TRACE_FILE ($TRACE_LINES lines)"

# ── Phase 5: Filter GR-relevant registers ──
log "Phase 5: Extracting PGRAPH / FECS / GPCCS register accesses"

GR_FILTER="$TRACE_DIR/k80_gr_${TIMESTAMP}_gr_regs.txt"
# FECS falcon:    0x409000-0x409FFF
# GPCCS broadcast: 0x41A000-0x41AFFF
# GR HUB:         0x400000-0x40FFFF
# GPC per-unit:   0x500000-0x53FFFF
# PGRAPH enable:  0x000200 (PMC_ENABLE)
# Method dispatch: 0x000260
# PRI ring:       0x120000-0x12FFFF
# Clock gating:   0x13C000-0x13CFFF (pxbar), 0x1B4000-0x1BFFFF (pcounter)
rg -i '004[01][0-9a-f]{4}\b|005[0-3][0-9a-f]{4}\b|00000200\b|00000260\b|0012[0-9a-f]{4}\b|0013[cC][0-9a-f]{3}\b|001[bB][0-9a-f]{4}\b' \
    "$TRACE_FILE" > "$GR_FILTER" 2>/dev/null || true

GR_LINES=$(wc -l < "$GR_FILTER" 2>/dev/null || echo 0)
ok "GR register filter: $GR_FILTER ($GR_LINES lines)"

# Focused filters for boot analysis
FECS_FILTER="$TRACE_DIR/k80_gr_${TIMESTAMP}_fecs.txt"
rg -i '00409[0-9a-f]{3}\b' "$TRACE_FILE" > "$FECS_FILTER" 2>/dev/null || true
FECS_LINES=$(wc -l < "$FECS_FILTER" 2>/dev/null || echo 0)
ok "FECS filter: $FECS_FILTER ($FECS_LINES lines)"

GPCCS_FILTER="$TRACE_DIR/k80_gr_${TIMESTAMP}_gpccs.txt"
rg -i '0041[aA][0-9a-f]{3}\b' "$TRACE_FILE" > "$GPCCS_FILTER" 2>/dev/null || true
GPCCS_LINES=$(wc -l < "$GPCCS_FILTER" 2>/dev/null || echo 0)
ok "GPCCS filter: $GPCCS_FILTER ($GPCCS_LINES lines)"

CLKGATE_FILTER="$TRACE_DIR/k80_gr_${TIMESTAMP}_clkgate.txt"
rg -i '0041[aA]89[04]\b|00418504\b|0041860[cC]\b|0041868[cC]\b' "$TRACE_FILE" > "$CLKGATE_FILTER" 2>/dev/null || true
CLKGATE_LINES=$(wc -l < "$CLKGATE_FILTER" 2>/dev/null || echo 0)
ok "Clock gating filter: $CLKGATE_FILTER ($CLKGATE_LINES lines)"

# Summary
SUMMARY="$TRACE_DIR/k80_gr_${TIMESTAMP}_summary.txt"
{
    echo "=== K80 GR Init mmiotrace summary ==="
    echo "BDF: $BDF ($MODE)"
    echo "Date: $(date)"
    echo "Total trace lines: $TRACE_LINES"
    echo "GR-relevant lines: $GR_LINES"
    echo "FECS lines: $FECS_LINES"
    echo "GPCCS lines: $GPCCS_LINES"
    echo "Clock gating lines: $CLKGATE_LINES"
    echo ""
    echo "=== FECS CPUCTL (0x409100) writes ==="
    rg -i '409100' "$FECS_FILTER" 2>/dev/null || echo "(none)"
    echo ""
    echo "=== FECS ITFEN (0x409048) writes ==="
    rg -i '409048' "$FECS_FILTER" 2>/dev/null || echo "(none)"
    echo ""
    echo "=== FECS DMACTL (0x40910C) writes ==="
    rg -i '40910[cC]' "$FECS_FILTER" 2>/dev/null || echo "(none)"
    echo ""
    echo "=== FECS BOOTVEC (0x409104) writes ==="
    rg -i '409104' "$FECS_FILTER" 2>/dev/null || echo "(none)"
    echo ""
    echo "=== FECS IMEMC (0x409180) writes ==="
    rg -i '409180' "$FECS_FILTER" 2>/dev/null | head -5 || echo "(none)"
    echo ""
    echo "=== GPCCS CPUCTL (0x41A100) writes ==="
    rg -i '41[aA]100' "$GPCCS_FILTER" 2>/dev/null || echo "(none)"
    echo ""
    echo "=== GPCCS ITFEN (0x41A048) writes ==="
    rg -i '41[aA]048' "$GPCCS_FILTER" 2>/dev/null || echo "(none)"
    echo ""
    echo "=== GPCCS DMACTL (0x41A10C) writes ==="
    rg -i '41[aA]10[cC]' "$GPCCS_FILTER" 2>/dev/null || echo "(none)"
    echo ""
    echo "=== GPCCS BOOTVEC (0x41A104) writes ==="
    rg -i '41[aA]104' "$GPCCS_FILTER" 2>/dev/null || echo "(none)"
    echo ""
    echo "=== GPCCS IMEMC (0x41A180) writes ==="
    rg -i '41[aA]180' "$GPCCS_FILTER" 2>/dev/null | head -5 || echo "(none)"
    echo ""
    echo "=== GPC0 GPCCS (0x502xxx) writes ==="
    rg -i '00502[0-9a-f]{3}\b' "$GR_FILTER" 2>/dev/null | head -20 || echo "(none)"
    echo ""
    echo "=== PMC_ENABLE (0x200) writes ==="
    rg -i '00000200\b' "$GR_FILTER" 2>/dev/null || echo "(none)"
    echo ""
    echo "=== Method dispatch (0x260) writes ==="
    rg -i '00000260\b' "$GR_FILTER" 2>/dev/null || echo "(none)"
    echo ""
    echo "=== CTXSW_MAILBOX0 (0x409800) writes ==="
    rg -i '409800' "$FECS_FILTER" 2>/dev/null || echo "(none)"
    echo ""
    echo "=== Clock gating (GPCCS BLCG 0x41A890 / SLCG 0x41A894) ==="
    cat "$CLKGATE_FILTER" 2>/dev/null || echo "(none)"
} > "$SUMMARY"

ok "Summary: $SUMMARY"

# ── Phase 6: Cleanup — swap back to vfio-pci ──
log "Phase 6: Returning $BDF to vfio-pci"

if [[ -L "$SYSFS/driver" ]]; then
    DRV_CUR="$(basename "$(readlink "$SYSFS/driver")")"
    echo "$BDF" > "/sys/bus/pci/drivers/$DRV_CUR/unbind" 2>/dev/null || true
    sleep 1
fi

echo "vfio-pci" > "$SYSFS/driver_override" 2>/dev/null || true
echo "$BDF" > /sys/bus/pci/drivers/vfio-pci/bind 2>/dev/null || \
    echo "$BDF" > /sys/bus/pci/drivers_probe 2>/dev/null || true

DRV_FINAL="(none)"
[[ -L "$SYSFS/driver" ]] && DRV_FINAL="$(basename "$(readlink "$SYSFS/driver")")"
log "Final driver: $DRV_FINAL"

rmmod nouveau 2>/dev/null || true

echo ""
ok "════════════════════════════════════════════════════════"
ok "  K80 GR Init mmiotrace capture complete"
ok "  Full trace: $TRACE_FILE"
ok "  GR filter:  $GR_FILTER"
ok "  FECS:       $FECS_FILTER"
ok "  GPCCS:      $GPCCS_FILTER"
ok "  Summary:    $SUMMARY"
ok "════════════════════════════════════════════════════════"
