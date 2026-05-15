#!/usr/bin/env bash
# Runs the parallel-scaling bench once per thread count in a separate process.
# A fresh process per N is required because TaskPool is a process-global singleton.
set -euo pipefail

for n in 1 2 4 8; do
    MCRS_BENCH_THREADS=$n cargo bench --bench parallel_scaling --features mcrs_minecraft_lighting/bench-helpers
done
