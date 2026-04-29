#!/usr/bin/env bash
# k80_pmu_io_pgob.sh — PGOB via PMU falcon I/O data port
#
# Instead of direct BAR0 MMIO to 0x0205xx (which generates PRIVRING faults
# on GK210B), this script writes the PGOB power steps through the PMU
# falcon's data port at 0x10a1c0/0x10a1c4. The falcon's internal I/O bus
# may have a different routing path than the host PRI ring.
#
# The PMU falcon data port works like this:
#   1. Write target address to 0x10a1c0 (with direction bits)
#      - Bit 24 = 1: data segment (DMEM)
#      - For I/O access, use the falcon IBUS mechanism
#   2. Read/write data via 0x10a1c4
#
# However, the falcon data port accesses DMEM, not arbitrary GPU registers.
# To access GPU registers through the falcon, we need to use the falcon's
# MMIO window (I/O register access), which is at falcon_base + 0x700-0x7FF
# on some falcon versions, or through the falcon's external I/O mechanism.
#
# On GK110, the PMU falcon has an "I/O access" mechanism:
#   - 0x10a7a0 = PMU_FALCON_IORDATA (I/O register read data)
#   - 0x10a7a4 = PMU_FALCON_IOWDATA (I/O register write data — used for wr32 trap)
#   - The falcon firmware can issue I/O reads/writes to arbitrary GPU registers
#
# From the HOST side, we can potentially trigger I/O operations by:
#   1. Loading a minimal PMU firmware that reads/writes PPWR registers
#   2. Or using the falcon debug interface
#
# This script tries multiple falcon I/O mechanisms to reach PPWR 0x0205xx.
#
# Usage:
#   sudo ./k80_pmu_io_pgob.sh [BDF]

set -euo pipefail

BDF="${1:-0000:4d:00.0}"
BAR0="/sys/bus/pci/devices/$BDF/resource0"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; NC='\033[0m'
log()  { echo -e "${CYAN}[pmu-io]${NC} $*"; }
ok()   { echo -e "${GREEN}[OK]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
fail() { echo -e "${RED}[FAIL]${NC} $*"; }

[[ $EUID -eq 0 ]] || { fail "Must run as root"; exit 1; }
[[ -f "$BAR0" ]] || { fail "$BAR0 not found"; exit 1; }

log "╔══════════════════════════════════════════════════════════╗"
log "║  K80 PMU Falcon I/O PGOB — GK210B (BDF: $BDF)   ║"
log "╚══════════════════════════════════════════════════════════╝"

python3 << 'PYEOF'
import mmap, struct, time, sys

BDF = sys.argv[1] if len(sys.argv) > 1 else "0000:4d:00.0"
BAR0 = f"/sys/bus/pci/devices/{BDF}/resource0"

f = open(BAR0, "r+b")
mm = mmap.mmap(f.fileno(), 0x1000000)

def rd(reg):
    mm.seek(reg)
    return struct.unpack("<I", mm.read(4))[0]

def wr(reg, val):
    mm.seek(reg)
    mm.write(struct.pack("<I", val & 0xFFFFFFFF))

def rd_pmu(offset):
    """Read PMU falcon register (base 0x10a000)"""
    return rd(0x10a000 + offset)

def wr_pmu(offset, val):
    """Write PMU falcon register (base 0x10a000)"""
    wr(0x10a000 + offset, val)

def log(msg):
    print(f"  {msg}", flush=True)

def check_gpcs():
    gpc0 = rd(0x500000)
    nst = rd(0x120070)
    return gpc0, nst, (gpc0 != 0xbadf1100 and gpc0 != 0 and gpc0 != 0xbad0011f)

# ── Baseline ──
boot0 = rd(0)
pmc = rd(0x200)
pmu_cpuctl = rd_pmu(0x100)
gpc0, nst, alive = check_gpcs()
log(f"BOOT0={boot0:#010x} PMC={pmc:#010x} PMU_CPUCTL={pmu_cpuctl:#010x}")
log(f"GPC0={gpc0:#010x} NSTATIONS={nst:#010x} alive={alive}")

if alive:
    log("GPCs already alive — nothing to do!")
    sys.exit(0)

# ══════════════════════════════════════════════════════════════
# METHOD 1: PMU falcon DMEM-based I/O write
#
# The PMU falcon DMEM port at 0x10a1c0/0x10a1c4 accesses DMEM.
# The falcon's I/O trap mechanism at 0x10a7a0/0x10a7a4 captures
# host writes and can be used to inject I/O operations.
#
# On Fermi+, the PMU falcon has a debug/IO window at:
#   FALCON_REG_BASE + 0x700 = I/O access port
# For PMU at 0x10a000: debug port = 0x10a700
# ══════════════════════════════════════════════════════════════

log("")
log("═══ METHOD 1: Falcon Debug I/O Port ═══")
log("Checking PMU falcon debug port at 0x10a700...")

for offset in range(0x700, 0x740, 4):
    val = rd_pmu(offset)
    if val != 0:
        log(f"  PMU+{offset:#06x} = {val:#010x}")

# Check if there's an external I/O window at 0x10a900+ range
log("Scanning PMU extended registers...")
for offset in [0x900, 0x904, 0x908, 0x90C, 0x910, 0xA00, 0xA04, 0xA08,
               0xB00, 0xB04, 0xB08, 0xB80, 0xB84, 0xB88]:
    val = rd_pmu(offset)
    if val != 0:
        log(f"  PMU+{offset:#06x} = {val:#010x}")

# ══════════════════════════════════════════════════════════════
# METHOD 2: Write to PPWR via PMU falcon's IBUS (data segment)
#
# If the PMU falcon is halted, we can write to its DMEM, place
# I/O commands there, and start the falcon to execute them.
# This requires writing a minimal falcon program.
#
# Falcon instruction set (fuc4/fuc5):
#   iowr [reg], val  — write to external I/O register
#   iord val, [reg]  — read from external I/O register
#
# The falcon has a 32-bit data bus to the "I/O space" which maps
# GPU registers. The mapping depends on the falcon's I/O base
# configuration.
#
# For the PMU falcon on GK110:
#   I/O base = 0x000000 (direct GPU register addressing)
#   iowr [0x020520], 0x0c  — writes to GPU register 0x020520
# ══════════════════════════════════════════════════════════════

log("")
log("═══ METHOD 2: Minimal PMU Microcode PGOB ═══")

# Check PMU state
cpuctl = rd_pmu(0x100)
log(f"PMU CPUCTL = {cpuctl:#010x}")

if cpuctl & 0x20:
    log("PMU is STOPPED (power-on reset state)")
elif cpuctl & 0x10:
    log("PMU is HALTED")
else:
    log("PMU is RUNNING — halting first")
    wr_pmu(0x100, 0x00000010)  # HALT
    time.sleep(0.1)

# The falcon uses fuc (Falcon Microcode) ISA.
# We need a tiny program that:
#   1. Writes PGOB disable values to 0x0205xx registers
#   2. Polls for completion
#   3. Halts
#
# fuc4 instruction encoding:
#   iowr I[reg], Rx  — opcode F6, format: [reg_field][rx][F6]
#   mov Rx, #imm     — opcode varies
#   exit / halt       — opcode F8 (with subtype)
#
# Due to complexity of fuc encoding, use a pre-assembled approach:
# Write raw PGOB register values via falcon DMEM DMA if available.

# Actually, the simplest approach is to use the falcon's
# "I/O register window" if it exists. On some falcons:
#   FALCON_BASE + 0x0700 = FALCON_IOSPCE_SEL (I/O space selector)
#   FALCON_BASE + 0x0704 = FALCON_IOSPCE_ADDR (I/O address)
#   FALCON_BASE + 0x0708 = FALCON_IOSPCE_DATA (I/O data)
#
# This is documented in envytools as the "falcon external I/O" mechanism.

log("Trying falcon I/O space write mechanism...")

# Read current state of I/O space registers
io_sel = rd_pmu(0x700)
io_addr = rd_pmu(0x704)
io_data = rd_pmu(0x708)
log(f"  IO_SEL={io_sel:#010x} IO_ADDR={io_addr:#010x} IO_DATA={io_data:#010x}")

# Try writing PPWR register through falcon I/O space
# Set I/O address to 0x020520 and write 0x0000000c
log("  Writing 0x0000000c to 0x020520 via falcon I/O...")
wr_pmu(0x704, 0x020520)  # Set I/O address
wr_pmu(0x708, 0x0000000c)  # Write data
time.sleep(0.01)

# Read back to check if it took effect
wr_pmu(0x704, 0x020520)  # Set address for read
readback = rd_pmu(0x708)  # Read data
log(f"  Readback from 0x020520 via falcon I/O: {readback:#010x}")
host_read = rd(0x020520)
log(f"  Host BAR0 read of 0x020520: {host_read:#010x}")

# ══════════════════════════════════════════════════════════════
# METHOD 3: CRC/debug register scan for power domain control
#
# GK210B may have power domain registers at non-standard addresses.
# Scan the power management regions for responsive registers that
# differ from GK110.
# ══════════════════════════════════════════════════════════════

log("")
log("═══ METHOD 3: Power Domain Register Scan ═══")

# The proprietary driver may use registers in the 0x022xxx range
# (PPWR_NEW) or 0x0206xx range for GK210B-specific power control.
# Scan these ranges for non-zero, non-fault values.

ranges = [
    ("PPWR_0200", 0x020200, 0x020300),
    ("PPWR_0500", 0x020500, 0x020600),
    ("PPWR_0600", 0x020600, 0x020700),
    ("PWR_022", 0x022000, 0x022100),
    ("PWR_088", 0x088000, 0x088100),
]

for name, start, end in ranges:
    live = []
    for addr in range(start, end, 4):
        val = rd(addr)
        if val != 0 and val != 0xbadf1100 and val != 0xbadf1002 and val != 0xbad0011f and val != 0xffffffff and val != 0xbadf5040:
            live.append((addr, val))
    if live:
        log(f"  {name}: {len(live)} live registers")
        for addr, val in live[:10]:
            log(f"    {addr:#010x} = {val:#010x}")
    else:
        log(f"  {name}: no live registers")

# ══════════════════════════════════════════════════════════════
# METHOD 4: PRAMIN/VBIOS dump for DEVINIT analysis
#
# Read the VBIOS header from PRAMIN (GPU internal memory mapped
# to BAR0) to find DEVINIT tables.
# ══════════════════════════════════════════════════════════════

log("")
log("═══ METHOD 4: VBIOS Header Probe ═══")

# VBIOS is typically at PRAMIN window (0x700000) or through a
# PCI expansion ROM. On Kepler, PRAMIN starts at the address
# configured in NV_PMC_VBIOS_ROM (or similar).
#
# Check several possible VBIOS locations
for vbios_base_name, vbios_base in [
    ("PRAMIN_0x700000", 0x700000),
    ("PRAMIN_0x300000", 0x300000),
    ("PCI_ROM_0xC0000", 0xC0000),
]:
    try:
        sig0 = rd(vbios_base)
        sig1 = rd(vbios_base + 4)
        header = f"{sig0:#010x} {sig1:#010x}"
        # VBIOS signature: 0xAA55 at offset 0
        if (sig0 & 0xFFFF) == 0xAA55:
            log(f"  {vbios_base_name}: VBIOS FOUND! sig={header}")
            # Read PCIR offset
            pcir_offset_word = rd(vbios_base + 0x18)
            pcir_offset = pcir_offset_word & 0xFFFF
            log(f"    PCIR offset: {pcir_offset:#06x}")
            if pcir_offset < 0x1000:
                pcir = rd(vbios_base + pcir_offset)
                log(f"    PCIR sig: {pcir:#010x} (expect 0x52494350 = 'PCIR')")
        else:
            log(f"  {vbios_base_name}: no VBIOS (got {header})")
    except Exception as e:
        log(f"  {vbios_base_name}: error — {e}")

# ── Final check ──
log("")
gpc0, nst, alive = check_gpcs()
log(f"FINAL: GPC0={gpc0:#010x} NSTATIONS={nst:#010x} alive={alive}")

if alive:
    log("*** GPCs ALIVE! ***")
    sys.exit(0)
else:
    log("GPCs still gated. Falcon I/O approach needs further investigation.")
    log("Next: nvidia-470 mmiotrace to find the actual power sequence.")
    sys.exit(1)

mm.close()
f.close()
PYEOF
