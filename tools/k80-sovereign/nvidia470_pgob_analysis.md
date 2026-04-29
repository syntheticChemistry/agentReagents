# nvidia-470 PGOB Binary Analysis — GK210B (Tesla K80)

## Source

Static disassembly of `nv-kernel.o_binary` from `NVIDIA-Linux-x86_64-470.256.02.run`.
The proprietary module was also successfully compiled for kernel 6.17 and tested
in a QEMU VM with K80 VFIO passthrough (probe succeeded).

## Key Finding

nvidia-470 uses a **PSW-only PGOB handshake** on register `0x10a78c`, completely
bypassing the `0x0205xx` power domain registers that Nouveau's `gk110_pmu_pgob()`
uses. This is significant because the `0x0205xx` registers cause PRIVRING faults
on GK210B, which is the root cause of our PGOB failure.

## Register: 0x10a78c (PMU PGOB PSW Control)

| Bit | Name     | Description                                      |
|-----|----------|--------------------------------------------------|
| 0   | TRIGGER  | Set to execute PSW command, then clear            |
| 1   | STATE    | 1 = request power-gated, 0 = request ungated     |

## Disassembled Functions

### `_nv029216rm` — PGOB Disable (Ungate GPCs)

Address: `0x634010` in `nv-kernel.o_binary`

```
1. val = read(0x10a78c)
2. write(0x10a78c, val & ~0x02)    # Clear bit 1 → request ungated
3. val = read(0x10a78c)
4. write(0x10a78c, val | 0x01)     # Set bit 0 → trigger PSW
5. val = read(0x10a78c)
6. write(0x10a78c, val & ~0x01)    # Clear bit 0 → release trigger
```

### `_nv029114rm` — PGOB Enable (Power-gate GPCs)

Address: `0x633f30` in `nv-kernel.o_binary`

```
1. val = read(0x10a78c)
2. write(0x10a78c, val | 0x02)     # Set bit 1 → request power-gated
3. val = read(0x10a78c)
4. write(0x10a78c, val | 0x01)     # Set bit 0 → trigger PSW
5. val = read(0x10a78c)
6. write(0x10a78c, val & ~0x01)    # Clear bit 0 → release trigger
```

### Pre-requisites (from `_nv029114rm`)

Before the PGOB sequence, the function calls a vtable method at offset `0x558`
with `edx=0x1`, and another at `0x4f0` with `edx=0x100`. These are likely:
- Power management state transitions
- PMC_ENABLE bit toggles (PMU must be powered on)

## Difference from Nouveau

| Aspect           | Nouveau `gk110_pmu_pgob()`  | nvidia-470 `_nv029216rm`    |
|------------------|-----------------------------|-----------------------------|
| PMC bit 12       | Toggled (disable/enable GR) | Not touched                 |
| PMC bit 27       | 0→1 transition required     | Not touched                 |
| 0x10a78c         | Set bit 1, pulse bit 0      | Clear bit 1, pulse bit 0   |
| 0x0206b4         | NOP mask sync               | Not touched                 |
| 0x0205xx steps   | 16-step power domain seq    | **Not used at all**         |
| 0x10a78c cleanup | Clear bit 1, pulse bit 0    | (same as main sequence)     |
| PMC restore      | Clear bit 27, set bit 12    | Not touched                 |

## Implementation

Added to `coral-driver/src/nv/vfio_compute/pgob.rs` as `nvidia470_pgob_disable()`.
The `kepler_warm.rs` init path now tries this PSW-only sequence first, falling
back to the Nouveau sequence if GPCs remain gated.

## Date

2026-04-29
