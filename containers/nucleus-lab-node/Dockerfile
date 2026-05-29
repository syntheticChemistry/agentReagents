# SPDX-License-Identifier: AGPL-3.0-or-later
#
# nucleus-lab-node — Reusable container image for benchScale NUCLEUS labs
#
# Pre-bakes all tools needed by benchScale topologies so that lab scripts
# don't install packages at runtime. Supports:
#   - Network condition simulation (tc netem via iproute2)
#   - UDS health checks (socat)
#   - Primal binary execution (musl-static, no glibc needed)
#   - plasmidBin deploy pipeline (bash, ssh, curl, jq)
#   - MitoBeacon seed generation (xxd for hex encoding)
#   - systemd service management (systemd is PID 1)
#
# Usage in topology YAML:
#   image: nucleus-lab-node
#
# Build:
#   docker build -t nucleus-lab-node containers/nucleus-lab-node/
#
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    bash \
    ca-certificates \
    curl \
    git \
    iproute2 \
    iputils-ping \
    jq \
    openssh-client \
    openssh-server \
    procps \
    python3 \
    socat \
    xxd \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p \
    /opt/plasmidBin \
    /opt/membrane \
    /opt/ecoPrimals/primals \
    /opt/ecoPrimals/infra \
    /opt/ecoPrimals/gardens \
    /opt/ecoPrimals/springs \
    /run/membrane \
    /etc/membrane/family/nodes

RUN mkdir -p /run/sshd && \
    ssh-keygen -A

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
