#!/usr/bin/env bash
# Build proprietary nvidia-470.256.02 for kernel 6.17 (Pop!_OS)
# Safe: builds in /tmp, never installs to system
set -euo pipefail

NVIDIA_URL="https://us.download.nvidia.com/tesla/470.256.02/NVIDIA-Linux-x86_64-470.256.02.run"
BUILD_DIR="/tmp/nvidia-470-pure"
RUN_FILE="/tmp/NVIDIA-Linux-x86_64-470.256.02.run"

echo "=== nvidia-470 kernel module build for $(uname -r) ==="

if [ ! -f "$RUN_FILE" ]; then
    echo "Downloading .run installer..."
    wget -q --show-progress "$NVIDIA_URL" -O "$RUN_FILE"
fi

if [ ! -d "$BUILD_DIR" ]; then
    echo "Extracting..."
    chmod +x "$RUN_FILE"
    "$RUN_FILE" --extract-only --target "$BUILD_DIR"
fi

cd "$BUILD_DIR/kernel"

# Apply Pop!_OS compat patches (if available from DKMS source)
PATCH_DIR="/usr/src/nvidia-470.256.02/patches"
if [ -d "$PATCH_DIR" ]; then
    for p in "$PATCH_DIR"/buildfix_kernel_6.*.patch; do
        echo "Applying: $(basename "$p")"
        patch -p1 -N < "$p" 2>/dev/null || true
    done
fi

# Fix ccflags-y (kernel 6.17 dropped EXTRA_CFLAGS)
sed -i 's/EXTRA_CFLAGS/ccflags-y/g' Kbuild 2>/dev/null || true

# Fix del_timer_sync (kernel 6.2+)
if [ ! -f nvidia/nv_compat_617.h ]; then
    cat > nvidia/nv_compat_617.h << 'HEADER'
#ifndef NV_COMPAT_617_H
#define NV_COMPAT_617_H
#include <linux/version.h>
#if LINUX_VERSION_CODE >= KERNEL_VERSION(6,2,0)
#define del_timer_sync timer_delete_sync
#endif
#endif
HEADER
    grep -q 'nv_compat_617.h' nvidia/nv.c || sed -i '1i #include "nv_compat_617.h"' nvidia/nv.c
fi

# Ensure nv-kernel.o symlink
ln -sf nv-kernel.o_binary nvidia/nv-kernel.o

# Build
echo "Building..."
make clean 2>/dev/null; rm -rf conftest

CC="${CC:-x86_64-linux-gnu-gcc-12}"
/usr/bin/env -i HOME="$HOME" PATH="/usr/bin:/bin:/usr/sbin:/sbin" \
    make "CC=$CC" \
    "SYSSRC=/lib/modules/$(uname -r)/build" \
    NV_KERNEL_MODULES=nvidia \
    KCFLAGS="-Wno-error" \
    -j"$(nproc)"

if [ -f nvidia.ko ]; then
    echo "=== SUCCESS: nvidia.ko built ==="
    ls -lh nvidia.ko
    strings nvidia.ko | grep 'license=' | head -1
else
    echo "=== FAILED ==="
    exit 1
fi
