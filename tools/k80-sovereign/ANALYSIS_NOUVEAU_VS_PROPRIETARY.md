# K80 GPC Ungate: Nouveau vs Proprietary Driver Analysis

## Status: GK210B GPCs remain power-gated under all host-side MMIO approaches

Date: 2026-04-28

## The Problem

On Tesla K80 (GK210B), GPCs are hardware power-gated at cold boot:
- `GPC0 @ 0x500000 = 0xbadf1100` (PRI fault — gated)
- `NSTATIONS = 1` (only ring master responds)
- `NGPC = 6` (ring master KNOWS about 6 GPCs from fuses/VBIOS)
- `GR_HUB @ 0x400000 = 0xbadf1002` (PRI fault)

All GPC-resident PGRAPH resources are unreachable. FECS/GPCCS firmware
cannot boot because the GPC stations are physically off.

## Nouveau's Approach (gk110_pmu_pgob — FAILS on GK210B)

**Source**: `drivers/gpu/drm/nouveau/nvkm/subdev/pmu/gk110.c`

Called from `gf100_gr_oneinit()` and `gf100_gr_init()`:
```c
nvkm_pmu_pgob(device->pmu, false);
gr->gpc_nr = nvkm_rd32(device, 0x409604);  // <-- FAULTS, GPCs gated
```

The `gk110_pmu_pgob()` function does:
1. Clear PMC bit 12 (disable PGRAPH)
2. Set PMC bit 27 (PGOB enable)
3. Sleep 50ms
4. PSW handshake: set/toggle bits in `0x10a78c` (PMU PSW register)
5. NOP mask write to `0x0206b4`
6. **16 power steps**: write pattern values to `0x020520`, `0x020524`,
   `0x02052c`, `0x020528`, `0x020530` — poll bit 31 of each for completion
7. Second PSW handshake
8. Clear PMC bit 27, re-enable PMC bit 12

### Why it fails on GK210B

**dmesg evidence:**
```
bus: MMIO write of fffffffc FAULT at 020520 [ PRIVRING ]
bus: MMIO write of fffffffe FAULT at 020524 [ PRIVRING ]
bus: MMIO write of fffffffc FAULT at 020524 [ PRIVRING ]
bus: MMIO write of fffffff8 FAULT at 020524 [ PRIVRING ]
bus: MMIO write of ffffffe0 FAULT at 020524 [ PRIVRING ]
bus: MMIO write of fffffffe FAULT at 020530 [ PRIVRING ]
bus: MMIO write of fffffffa FAULT at 02052c [ PRIVRING ]
```

All PPWR power-step writes generate PRIVRING faults. The GPU's PRI
ring reports them as errors, though BAR0 reads DO return non-zero values
(likely cached or aliased responses, not actual register state).

### Probe Results (k80_gpc_ungate_probe.sh)

**Register accessibility scan:**
- PPWR `0x020520` reads `0x0000000c` — responds but may be aliased
- PPWR `0x020524` reads `0x00000020`
- THERM `0x020004` reads `0x41110000` — THERM alive
- 33 live registers found in `0x020000-0x020100` range
- All PMU I/O registers (`0x10a780-0x10a7a0`) are zero

**Every approach tried:**
| Approach | Description | Result |
|----------|-------------|--------|
| A | GK104-style THERM_CTRL1 (0x020004) | FAIL — GPCs gated |
| B | PGRAPH reset + PRI re-enumerate | FAIL — NSTATIONS=1 |
| C | Full PMC engine cycle | FAIL — NSTATIONS=1 |
| D | Manual PMU falcon start | FAIL — PMU stays at CPUCTL=0x20 |
| E | PMC bit 27 toggle | FAIL — GPCs gated |
| F | Variant power-step patterns | FAIL — GPCs gated |
| SBR | Secondary Bus Reset | Puts GPU in cold boot (PMC=0xc0002020), needs DEVINIT |

## Proprietary Driver Differences (nvidia-470)

nvidia-470 (the last branch supporting Kepler) **succeeds** at initializing
GPCs on K80. Based on RE analysis, the likely differences are:

### 1. PMU Firmware-Mediated Power Management

The proprietary driver loads a full-featured PMU firmware blob that handles:
- PGOB (Power Gating On Board) via falcon local bus
- Voltage/clock domain management
- Thermal protection
- Board-level power sequencing

**Key difference**: Nouveau's `gk110_pmu_pgob()` does DIRECT HOST MMIO to
`0x0205xx` (CPU → BAR0 → PRI ring → PPWR). nvidia-470 sends a PGOB
command to the PMU falcon firmware, which accesses PPWR registers through
its own internal I/O path (falcon data port at `0x10a1c0-0x10a1c4`),
potentially bypassing the broken PRI ring routing.

Evidence:
- PMU falcon registers at `0x10Axxx` ARE accessible (CPUCTL, MAILBOX readable)
- PMU data port (`0x10a1c0-0x10a1c4`) can read/write arbitrary GPU registers
  through the falcon's I/O bus
- PPWR at `0x0205xx` faults through host BAR0 but may work through falcon I/O

### 2. GK210B-Specific VBIOS DEVINIT

The K80's VBIOS contains DEVINIT tables specific to GK210B, including:
- Board-specific power domain configuration
- PLX PCIe switch setup
- Dual-die power sequencing
- I2C VRM (voltage regulator module) configuration

nvidia-470 fully executes all DEVINIT tables. Nouveau's DEVINIT parser may
not implement all GK210B-specific opcodes.

### 3. I2C Board Power Controller

Tesla cards often have external power management ICs accessible via I2C:
- VRM controllers (e.g., TI/IR power ICs) for per-die voltage rails
- Board management controller for dual-die coordination
- GPIO-based power domain enables

nvidia-470 may use I2C to directly enable GPC power domains that are
controlled by the K80's board-level power management, not GPU-internal
PPWR registers.

## Proposed Solutions

### Solution A: nvidia-470 mmiotrace (RECOMMENDED FIRST)

Capture the proprietary driver's exact register sequence:
1. Install `nvidia-driver-470` (available: `470.256.02-0ubuntu0.22.04.1`)
2. Run `k80_nvidia470_mmiotrace.sh` to capture full init
3. Filter for PGOB/power/PRI registers
4. Compare with Nouveau's sequence

**Risk**: Temporarily disrupts RTX 5060 (nvidia-580). Needs careful
driver swap.

### Solution B: PMU Falcon I/O Boot (NO DRIVER SWAP)

Write PGOB power steps through the PMU falcon's data port instead of
direct BAR0 MMIO:
1. Use falcon data port (`0x10a1c0` addr, `0x10a1c4` data) to write
   `0x0205xx` registers from the falcon's I/O bus
2. This bypasses the host PRI ring and uses the falcon's internal routing
3. May need PMU to be in a specific state (halted with IMEM loaded)

**Implementation**: New reagent script `k80_pmu_io_pgob.sh`

### Solution C: VBIOS DEVINIT Dump + Analysis

Extract and analyze K80 VBIOS DEVINIT tables:
1. Read VBIOS from BAR0 (PRAMIN at `0x700000` or `0x300000`)
2. Parse with envytools `nvbios` or manual hex analysis
3. Find GK210B-specific power init sequences
4. Implement missing sequences in coral-driver

### Solution D: Nouveau Livepatch for PMU-Mediated PGOB

Replace `gk110_pmu_pgob()` with a version that:
1. Loads PMU firmware first
2. Sends PGOB command via PMU mailbox
3. Waits for PMU to complete power ungating
4. Then proceeds with GR init

This is the sovereign long-term fix for coral-driver.

## Key Register Map

```
HOST BAR0 MMIO PATH (broken for PPWR on GK210B):
  CPU → PCIe BAR0 → GPU PRI Ring → PPWR @ 0x0205xx → FAULT

PMU FALCON I/O PATH (potentially working):
  CPU → 0x10a1c0/0x10a1c4 → PMU falcon data port → internal I/O bus → PPWR

DEVINIT PATH:
  VBIOS ROM → DEVINIT engine → direct hardware → power domains
```

## Files

- `k80_gpc_ungate_probe.sh` — systematic PGOB probe (8 approaches)
- `k80_nvidia470_mmiotrace.sh` — nvidia-470 mmiotrace capture
- `k80_force_gr_init.c` — DRM channel tool (hits ENOSYS on 6.17.9)
- `livepatch_nvkm_mc_reset.c` — Nouveau teardown block (working)
- `README.md` — register reference and tool docs
