#!/usr/bin/env bash
# titanv_sec2_mmiotrace.sh — Capture mmiotrace of Titan V SEC2 boot via nouveau.
#
# Captures the full register-level trace of nouveau's SEC2 / ACR boot on Titan V
# (GV100). The resulting trace is filtered for SEC2 falcon registers (0x840xxx)
# and FBIF/instance-block (0x100cxx), providing the exact DMA and firmware upload
# sequence that coral-driver needs to replicate for sovereign SEC2 boot.
#
# Usage:
#   sudo ./titanv_sec2_mmiotrace.sh [BDF]
#   sudo TRACE_DIR=/tmp/traces ./titanv_sec2_mmiotrace.sh 0000:09:00.0

set -euo pipefail

BDF="${1:-0000:09:00.0}"
TRACE_DIR="${TRACE_DIR:-/tmp/coralreef_traces}"
TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
TRACE_FILE="$TRACE_DIR/titanv_sec2_${TIMESTAMP}.mmiotrace"
SETTLE_SECS="${SETTLE_SECS:-15}"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
log()  { echo -e "${CYAN}[titanv]${NC} $*"; }
ok()   { echo -e "${GREEN}[OK]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
fail() { echo -e "${RED}[FAIL]${NC} $*"; exit 1; }

[[ $EUID -eq 0 ]] || fail "Must run as root"
[[ -d "/sys/bus/pci/devices/$BDF" ]] || fail "$BDF not found on PCI bus"

mkdir -p "$TRACE_DIR"

# ── Phase 1: Preflight ──
log "Phase 1: Preflight — Titan V at $BDF"

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

# ── Phase 2: Enable mmiotrace ──
log "Phase 2: Enabling mmiotrace"

TRACING="/sys/kernel/debug/tracing"
if [[ ! -d "$TRACING" ]]; then
    mount -t debugfs debugfs /sys/kernel/debug 2>/dev/null || true
fi
[[ -d "$TRACING" ]] || fail "debugfs/tracing not available"

# Save current tracer
PREV_TRACER="$(cat "$TRACING/current_tracer" 2>/dev/null || echo nop)"

echo mmiotrace > "$TRACING/current_tracer"
echo 1 > "$TRACING/tracing_on"
ok "mmiotrace enabled"

# ── Phase 3: Load nouveau (SEC2 boot happens during probe) ──
log "Phase 3: Loading nouveau — SEC2/ACR boot will be captured"

# Clear driver_override
echo "" > "$SYSFS/driver_override" 2>/dev/null || true

modprobe --ignore-install nouveau 2>&1 || warn "modprobe nouveau returned error"
sleep 2

# Probe the specific device if nouveau didn't auto-claim it
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

# ── Phase 5: Filter SEC2-relevant registers ──
log "Phase 5: Extracting SEC2/ACR register accesses"

SEC2_FILTER="$TRACE_DIR/titanv_sec2_${TIMESTAMP}_sec2_regs.txt"
# SEC2 falcon: BAR0 + 0x840000-0x841FFF
# FBIF/DMA: 0x100C00-0x100CFF
# Instance block / PRAMIN: 0x1700-0x17FF, 0x700000+
# PMC: 0x200
rg -i '84[01][0-9a-f]{3}|100c[0-9a-f]{2}|00001[7][0-9a-f]{2}|00000200' "$TRACE_FILE" > "$SEC2_FILTER" 2>/dev/null || true

SEC2_LINES=$(wc -l < "$SEC2_FILTER" 2>/dev/null || echo 0)
ok "SEC2 register filter: $SEC2_FILTER ($SEC2_LINES lines)"

# Extract a summary of distinct register addresses written
SUMMARY="$TRACE_DIR/titanv_sec2_${TIMESTAMP}_summary.txt"
{
    echo "=== Titan V SEC2 mmiotrace summary ==="
    echo "BDF: $BDF"
    echo "Date: $(date)"
    echo "Total trace lines: $TRACE_LINES"
    echo "SEC2-relevant lines: $SEC2_LINES"
    echo ""
    echo "=== Distinct register addresses (writes) ==="
    rg '^W' "$SEC2_FILTER" 2>/dev/null | rg -oP '0x[0-9a-fA-F]+' | sort -u || true
    echo ""
    echo "=== SEC2 CPUCTL (0x840100) writes ==="
    rg '840100' "$SEC2_FILTER" 2>/dev/null || echo "(none found)"
    echo ""
    echo "=== SEC2 DMACTL (0x84010C) writes ==="
    rg '84010[cC]' "$SEC2_FILTER" 2>/dev/null || echo "(none found)"
    echo ""
    echo "=== SEC2 BOOTVEC (0x840104) writes ==="
    rg '840104' "$SEC2_FILTER" 2>/dev/null || echo "(none found)"
    echo ""
    echo "=== FBIF/instance-block writes ==="
    rg '100[cC]' "$SEC2_FILTER" 2>/dev/null || echo "(none found)"
} > "$SUMMARY"

ok "Summary: $SUMMARY"

# ── Phase 6: Cleanup ──
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
ok "  Titan V SEC2 mmiotrace capture complete"
ok "  Trace: $TRACE_FILE"
ok "  SEC2 filter: $SEC2_FILTER"
ok "  Summary: $SUMMARY"
ok "════════════════════════════════════════════════════════"
