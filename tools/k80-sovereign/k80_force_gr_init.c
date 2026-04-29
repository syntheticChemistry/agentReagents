// SPDX-License-Identifier: AGPL-3.0-or-later
//
// k80_force_gr_init — Force Nouveau to initialize the GR engine on headless K80.
//
// On headless Tesla K80 (GK210B), Nouveau never initializes the GR (Graphics/
// Compute) engine because no client requests it. The GR engine manages FECS/
// GPCCS Falcons and GPC power domains required for sovereign compute dispatch.
//
// This tool opens the K80's DRM render node and creates a FIFO channel bound
// to the GR engine, which triggers Nouveau's lazy initialization path:
//   nouveau_abi16_ioctl_channel_alloc → nouveau_channel_new →
//   nvkm_chan_new → nvkm_engine_init(GR) → gf100_gr_oneinit + gf100_gr_init
//
// After GR init, FECS/GPCCS firmware is loaded, GPCs are power-ungated via
// PGOB, and the full GR MMIO register space (0x400000-0x5FFFFF) becomes
// accessible via PRI.
//
// Usage:
//   ./k80_force_gr_init /dev/dri/renderD129
//   ./k80_force_gr_init /dev/dri/card1
//
// Build:
//   gcc -o k80_force_gr_init k80_force_gr_init.c $(pkg-config --cflags --libs libdrm_nouveau)
//
// Alternative (no pkg-config):
//   gcc -o k80_force_gr_init k80_force_gr_init.c -I/usr/include/libdrm -I/usr/include/libdrm/nouveau -ldrm_nouveau

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>
#include <errno.h>
#include <nouveau.h>

int main(int argc, char **argv)
{
    if (argc < 2) {
        fprintf(stderr, "Usage: %s /dev/dri/renderDN\n", argv[0]);
        return 1;
    }

    const char *node = argv[1];
    int fd = open(node, O_RDWR);
    if (fd < 0) {
        fprintf(stderr, "ERROR: open(%s): %s\n", node, strerror(errno));
        return 1;
    }
    printf("Opened %s (fd=%d)\n", node, fd);

    struct nouveau_drm *drm = NULL;
    int ret = nouveau_drm_new(fd, &drm);
    if (ret) {
        fprintf(stderr, "ERROR: nouveau_drm_new: %s (%d)\n", strerror(-ret), ret);
        close(fd);
        return 1;
    }
    printf("DRM version: %u, NVIF: %s\n", drm->version, drm->nvif ? "yes" : "no");

    struct nouveau_device *dev = NULL;
    ret = nouveau_device_new(&drm->client, 0, NULL, 0, &dev);
    if (ret) {
        fprintf(stderr, "ERROR: nouveau_device_new: %s (%d)\n", strerror(-ret), ret);
        nouveau_drm_del(&drm);
        close(fd);
        return 1;
    }
    printf("Device: chipset=0x%x VRAM=%lluMiB GART=%lluMiB\n",
           dev->chipset,
           (unsigned long long)dev->vram_size / (1024*1024),
           (unsigned long long)dev->gart_size / (1024*1024));

    // Query GRAPH_UNITS to see current topology (may be 0 if GR not initialized).
    uint64_t graph_units = 0;
    ret = nouveau_getparam(dev, 13 /* NOUVEAU_GETPARAM_GRAPH_UNITS */, &graph_units);
    printf("GRAPH_UNITS (pre-channel): 0x%llx (ret=%d)\n",
           (unsigned long long)graph_units, ret);

    // Create a GR-bound channel. This is what triggers gf100_gr_init().
    //
    // For Kepler (GK104+/NVE0+), engine selection goes into nve0_fifo.engine.
    // libdrm_nouveau translates this into tt_ctxdma_handle = engine mask
    // in the kernel ioctl.
    struct nve0_fifo nve0;
    memset(&nve0, 0, sizeof(nve0));
    nve0.engine = NVE0_FIFO_ENGINE_GR;

    struct nouveau_object *chan = NULL;
    ret = nouveau_object_new(&dev->object, 0, NOUVEAU_FIFO_CHANNEL_CLASS,
                             &nve0, sizeof(nve0), &chan);
    if (ret) {
        fprintf(stderr, "ERROR: channel alloc (GR engine): %s (%d)\n",
                strerror(-ret), ret);
        fprintf(stderr, "  This means Nouveau could not initialize the GR engine.\n");
        fprintf(stderr, "  Check dmesg for 'nouveau' errors.\n");
        nouveau_device_del(&dev);
        nouveau_drm_del(&drm);
        close(fd);
        return 1;
    }

    struct nouveau_fifo *fifo = (struct nouveau_fifo *)chan->data;
    printf("GR channel created: channel=%u pushbuf=%u\n",
           fifo->channel, fifo->pushbuf);

    // Re-query GRAPH_UNITS after channel creation.
    graph_units = 0;
    ret = nouveau_getparam(dev, 13, &graph_units);
    printf("GRAPH_UNITS (post-channel): 0x%llx (ret=%d)\n",
           (unsigned long long)graph_units, ret);
    if (graph_units != 0) {
        unsigned gpc_count = graph_units & 0xFFFF;
        unsigned tpc_mask = (graph_units >> 16) & 0xFFFF;
        printf("  GPC count: %u, TPC mask: 0x%04x\n", gpc_count, tpc_mask);
    }

    printf("GR engine initialized successfully. Keeping channel open for 2s...\n");
    sleep(2);

    printf("Cleaning up...\n");
    nouveau_object_del(&chan);
    nouveau_device_del(&dev);
    nouveau_drm_del(&drm);
    close(fd);

    printf("Done. GR engine should now be fully initialized in Nouveau.\n");
    return 0;
}
