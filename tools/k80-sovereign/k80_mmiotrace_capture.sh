#!/usr/bin/env bash
# k80_mmiotrace_capture.sh — Self-recovering nvidia-470 mmiotrace for K80.
#
# Installs itself as a systemd oneshot service, so it survives display manager
# shutdown and auto-restores the desktop on completion or failure.
#
# Usage: sudo ./k80_mmiotrace_capture.sh
# Output: /tmp/k80-mmiotrace/

set -euo pipefail

SCRIPT_PATH="$(readlink -f "$0")"
OUT="/tmp/k80-mmiotrace"

# If not running as the systemd service, install and start it
if [[ "${K80_MMIO_SERVICE:-}" != "1" ]]; then
    [[ $EUID -eq 0 ]] || { echo "Must be root"; exit 1; }

    mkdir -p "$OUT"

    cat > /etc/systemd/system/k80-mmiotrace.service << SVCEOF
[Unit]
Description=K80 mmiotrace capture (one-shot, self-recovering)
After=multi-user.target

[Service]
Type=oneshot
Environment=K80_MMIO_SERVICE=1
ExecStart=/bin/bash $SCRIPT_PATH
ExecStopPost=/bin/bash -c 'modprobe nvidia 2>/dev/null; modprobe nvidia_modeset 2>/dev/null; modprobe nvidia_drm 2>/dev/null; modprobe nvidia_uvm 2>/dev/null; sleep 2; systemctl start gdm 2>/dev/null || systemctl start gdm3 2>/dev/null || true'
TimeoutStartSec=180
StandardOutput=file:$OUT/capture.log
StandardError=file:$OUT/capture.log
SVCEOF

    systemctl daemon-reload
    echo "Service installed. Starting capture..."
    echo "Monitor: tail -f $OUT/capture.log"
    echo "Desktop will go dark for ~60 seconds."
    sleep 2
    systemctl start k80-mmiotrace.service
    # If we get here, service finished (or we got disconnected)
    echo "Service completed. Check $OUT/"
    exit 0
fi

# ── Running as systemd service below ──
NV470="/var/lib/dkms/nvidia/470.256.02/$(uname -r)/x86_64/module/nvidia.ko"
T="/sys/kernel/tracing"

echo "$(date): === K80 MMIOTRACE CAPTURE ==="
echo "nvidia-470: $NV470"

[[ -f "$NV470" ]] || { echo "FATAL: nvidia-470 not found"; exit 1; }

# Cold BAR0 snapshot
echo "$(date): Phase 0 — cold BAR0"
python3 -c "
import mmap,struct,json,os
for bdf in ['0000:4c:00.0','0000:4d:00.0']:
  res=f'/sys/bus/pci/devices/{bdf}/resource0'
  if not os.path.exists(res): continue
  try:
    fd=os.open(res,os.O_RDONLY|os.O_SYNC);mm=mmap.mmap(fd,0x1000000,mmap.MAP_SHARED,mmap.PROT_READ)
    def rd(r):mm.seek(r);return struct.unpack('<I',mm.read(4))[0]
    regs={n:f'{rd(a):#010x}' for n,a in [('BOOT0',0),('PMC',0x200),('PMU',0x10a100),('NSTA',0x120070),('GPC0',0x500000),('GR',0x400000),('FECS',0x409100),('PGOB',0x10a78c)]}
    mm.close();os.close(fd)
    json.dump(regs,open(f'$OUT/cold_{bdf[-7:].replace(\":\",\"\")}.json','w'),indent=2)
    print(f'{bdf}: {regs}')
  except Exception as e: print(f'{bdf}: {e}')
" || true

# Stop GDM
echo "$(date): Phase 1 — stopping GDM"
systemctl stop gdm 2>/dev/null || systemctl stop gdm3 2>/dev/null || true
sleep 3

# Kill any remaining GPU processes
echo "$(date): Phase 2 — killing GPU consumers"
fuser -k /dev/nvidia* 2>/dev/null || true
fuser -k /dev/dri/* 2>/dev/null || true
sleep 2

# Unload nvidia-580
echo "$(date): Phase 3 — unloading nvidia-580"
rmmod nvidia_uvm 2>/dev/null || true
rmmod nvidia_drm 2>/dev/null || true
rmmod nvidia_modeset 2>/dev/null || true
sleep 1
rmmod nvidia 2>/dev/null || true

if lsmod | grep -q '^nvidia '; then
    echo "$(date): nvidia still loaded, force attempt"
    fuser -k /dev/nvidia* 2>/dev/null || true
    sleep 2
    rmmod nvidia_uvm nvidia_drm nvidia_modeset nvidia 2>/dev/null || true
fi

if lsmod | grep -q '^nvidia '; then
    echo "$(date): FATAL — cannot unload nvidia-580"
    lsmod | grep nvidia
    exit 1
fi
echo "$(date): nvidia-580 unloaded"

# Enable mmiotrace
echo "$(date): Phase 4 — mmiotrace ON"
echo mmiotrace > "$T/current_tracer"
echo > "$T/trace"
echo 1 > "$T/tracing_on"

# Load nvidia-470
echo "$(date): Phase 5 — loading nvidia-470"
insmod "$NV470" NVreg_OpenRmEnableUnsupportedGpus=1 2>&1 || insmod "$NV470" 2>&1 || true
sleep 3

# Probe K80
echo "$(date): Phase 5b — probing K80"
for bdf in 0000:4c:00.0 0000:4d:00.0; do
    echo "$bdf" > /sys/bus/pci/drivers/nvidia/bind 2>/dev/null || true
done
sleep 5

# nvidia-smi for full init
echo "$(date): Phase 5c — nvidia-smi"
timeout 20 nvidia-smi 2>&1 | head -15 || echo "nvidia-smi timed out/failed"
sleep 3

# Stop mmiotrace
echo "$(date): Phase 6 — mmiotrace OFF, capturing"
echo 0 > "$T/tracing_on"
cp "$T/trace" "$OUT/mmiotrace_raw.log"
echo nop > "$T/current_tracer"
NLINES=$(wc -l < "$OUT/mmiotrace_raw.log")
echo "$(date): Raw trace: $NLINES lines"

# Warm BAR0 snapshot
echo "$(date): Phase 7 — warm BAR0"
python3 -c "
import mmap,struct,json,os
for bdf in ['0000:4c:00.0','0000:4d:00.0']:
  res=f'/sys/bus/pci/devices/{bdf}/resource0'
  if not os.path.exists(res): continue
  try:
    fd=os.open(res,os.O_RDONLY|os.O_SYNC);mm=mmap.mmap(fd,0x1000000,mmap.MAP_SHARED,mmap.PROT_READ)
    def rd(r):mm.seek(r);return struct.unpack('<I',mm.read(4))[0]
    regs={n:f'{rd(a):#010x}' for n,a in [('BOOT0',0),('PMC',0x200),('PMU',0x10a100),('NSTA',0x120070),('GPC0',0x500000),('GR',0x400000),('FECS',0x409100),('PGOB',0x10a78c)]}
    mm.close();os.close(fd)
    json.dump(regs,open(f'$OUT/warm_{bdf[-7:].replace(\":\",\"\")}.json','w'),indent=2)
    print(f'{bdf}: {regs}')
  except Exception as e: print(f'{bdf}: {e}')
" || true

# Dmesg
dmesg > "$OUT/dmesg.log" 2>/dev/null || true

# Filter
echo "$(date): Phase 8 — filtering"
grep -iE '0205[0-9a-f]{2}|10a[0-9a-f]{3}|000200|020004' "$OUT/mmiotrace_raw.log" > "$OUT/power_filter.log" 2>/dev/null || true
grep -iE '4[0-9a-f]{5}|5[0-3][0-9a-f]{4}|41[89a-f][0-9a-f]{3}' "$OUT/mmiotrace_raw.log" > "$OUT/gr_filter.log" 2>/dev/null || true
grep -iE '12[0-9a-f]{4}' "$OUT/mmiotrace_raw.log" > "$OUT/pri_filter.log" 2>/dev/null || true

echo "$(date): Power=$(wc -l < "$OUT/power_filter.log") GR=$(wc -l < "$OUT/gr_filter.log") PRI=$(wc -l < "$OUT/pri_filter.log")"

# Unload nvidia-470
echo "$(date): Phase 9 — unloading nvidia-470"
rmmod nvidia 2>/dev/null || true

echo "$(date): === CAPTURE COMPLETE ==="
ls -lh "$OUT/"

# ExecStopPost will restore nvidia-580 + GDM
