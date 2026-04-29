# K80 Sovereign GPU Pipeline — Reagent Tools

Tools for solving Tesla K80 (GK210B) sovereign VFIO compute dispatch via
`coralReef` / `coral-driver`.

## Problem Statement

GK210B GPCs are hardware power-gated at cold boot. Neither Nouveau nor our
Rust code can ungate them. The PRI ring discovers only 1 station (ring
master) because GPC/FBP stations are offline. All accesses to `0x5xxxxx`
(GPC) and `0x4xxxxx` (GR HUB) return `0xbadf1100` / `0xbadf1002` PRI faults.

The kernel's `gk110_pmu_pgob()` power steps at `0x0205xx` respond on GK210B
but do **not** actually ungate the GPCs — either the addresses differ from
GK110, or additional steps are needed.

## Key Finding: Nouveau Also Fails

dmesg during Nouveau load on headless K80 (kernel 6.17.9):
```
bus: MMIO read of 00000000 FAULT at 500000 [ PRIVRING ]
bus: MMIO read of 00000000 FAULT at 122924 [ PRIVRING ]
```

GR init runs (firmware loaded into FECS/GPCCS IMEM — 64KB each), but
`gf100_gr_init` returns failure because GPCs are inaccessible. The
`gf100_gr_fini` cleanup is blocked by our livepatch.

## What's Needed

An mmiotrace of **nvidia-470** (proprietary driver) initializing the K80.
The proprietary driver DOES successfully ungate GPCs. Comparing its register
sequence with `gk110_pmu_pgob()` will reveal the GK210B-specific difference.

## Tools

### `k80_gpc_ungate_probe.sh` (DIAGNOSTIC — run first)
Systematic probe that tries 8 different approaches to ungate GPCs:
A) GK104-style THERM, B) PGRAPH reset, C) PMC cycle, D) PMU start,
E) PMC bit 27 toggle, F) Power step variants, G) FLR/SBR, H) Register scan.
Produces comprehensive register dump.

### `k80_pmu_io_pgob.sh` (EXPERIMENTAL)
Attempts PGOB through the PMU falcon's I/O data port instead of direct
BAR0 MMIO. Scans falcon debug registers, I/O space, power domain
registers at non-standard addresses, and probes VBIOS location.

### `k80_nvidia470_mmiotrace.sh` (CAPTURE)
Captures full mmiotrace of nvidia-470 initializing K80. Filters output into:
- `power_filter.log` — PGOB registers (0x0205xx, 0x10a78c, PMC 0x200)
- `gr_filter.log` — GR engine (0x4xxxxx, 0x5xxxxx)
- `pri_filter.log` — PRI ring (0x12xxxx)
- `pre_registers.json` / `post_registers.json` — BAR0 snapshots

### `k80_force_gr_init.c` (TOOL)
libdrm_nouveau-based tool that creates a GR-bound DRM channel, triggering
Nouveau's lazy GR init. Currently hits ENOSYS due to libdrm/kernel NVIF
version mismatch on 6.17.9.

Build: `make` (requires `libdrm-dev`, `libdrm-nouveau-dev`)

### `livepatch_nvkm_mc_reset.c` (LIVEPATCH)
Blocks `gf100_gr_fini`, `nvkm_pmu_fini`, `nvkm_mc_disable` during Nouveau
teardown to preserve warm state for VFIO handoff.

Build: `cd hotSpring/scripts/livepatch && make`

### `ANALYSIS_NOUVEAU_VS_PROPRIETARY.md` (DOCS)
Full analysis of Nouveau vs nvidia-470 differences, probe results, and
proposed solutions ranked by feasibility.

## Register Quick Reference

| Register       | Address    | Purpose                              |
|---------------|------------|--------------------------------------|
| PMC_ENABLE     | 0x200      | Engine power domains (bit 12=PGRAPH) |
| PMC bit 27     | 0x200[27]  | PGOB gate enable                     |
| PMU_PGOB_PSW   | 0x10a78c   | PMU power-switch handshake           |
| PPWR_PGOB[0]   | 0x020520   | Power domain step 0                  |
| PPWR_PGOB[1]   | 0x020524   | Power domain steps 1-4               |
| PPWR_PGOB[2]   | 0x02052c   | Power domain steps 6-13              |
| PPWR_PGOB[3]   | 0x020528   | Power domain steps 14-15             |
| PPWR_PGOB[4]   | 0x020530   | Power domain step 5                  |
| PPWR_SYNC      | 0x0206b4   | NOP mask (hardware sync)             |
| PRI_RING_CMD   | 0x12004c   | Ring master command (0x02=ACK, 0x04=enum) |
| PRI_RING_STAT  | 0x120058   | Ring status (bit 31=busy)            |
| PRI_NSTATIONS  | 0x120070   | Discovered stations (should be >1)   |
| GPC0_ID        | 0x500000   | GPC0 identity (0xbadf1100 when gated) |
| GR_HUB         | 0x400000   | GR HUB base (0xbadf1002 when gated)  |
