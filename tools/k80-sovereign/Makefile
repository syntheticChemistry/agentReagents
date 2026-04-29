# SPDX-License-Identifier: AGPL-3.0-or-later
# Build tools for K80 sovereign GPU pipeline.

CC ?= gcc-12
CFLAGS ?= -Wall -Wextra -O2
PKG_CFLAGS := $(shell pkg-config --cflags libdrm_nouveau 2>/dev/null)
PKG_LIBS := $(shell pkg-config --libs libdrm_nouveau 2>/dev/null)

# Fallback if pkg-config not available
ifeq ($(PKG_CFLAGS),)
	PKG_CFLAGS = -I/usr/include/libdrm -I/usr/include/libdrm/nouveau
	PKG_LIBS = -ldrm_nouveau
endif

TARGETS = k80_force_gr_init

all: $(TARGETS)

k80_force_gr_init: k80_force_gr_init.c
	$(CC) -B/usr/lib/gcc/x86_64-linux-gnu/12 $(CFLAGS) $(PKG_CFLAGS) -o $@ $< $(PKG_LIBS)

clean:
	rm -f $(TARGETS)

.PHONY: all clean
