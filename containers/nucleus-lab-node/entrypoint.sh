#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# nucleus-lab-node entrypoint — applies network conditions from env vars,
# then keeps the container alive for benchScale lab orchestration.

apply_network_conditions() {
    local latency="${NETEM_LATENCY_MS:-0}"
    local jitter="${NETEM_JITTER_MS:-0}"
    local loss="${NETEM_LOSS_PCT:-0}"
    local rate="${NETEM_RATE_KBPS:-0}"

    if [[ "$latency" != "0" ]] || [[ "$loss" != "0" ]]; then
        local cmd="tc qdisc add dev eth0 root netem"
        [[ "$latency" != "0" ]] && cmd+=" delay ${latency}ms"
        [[ "$jitter" != "0" ]]  && cmd+=" ${jitter}ms"
        [[ "$loss" != "0" ]]    && cmd+=" loss ${loss}%"
        [[ "$rate" != "0" ]]    && cmd+=" rate ${rate}kbit"
        eval "$cmd" 2>/dev/null || echo "[entrypoint] tc netem not available (need NET_ADMIN)"
    fi
}

apply_network_conditions

exec sleep infinity
