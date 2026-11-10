# Devices

Summaries of `tools/perf-kit` runs (`run.sh --full`) on the machines of trusted testers, one block per run, checked by hand before they are added. Method and caveats: `docs/perf/reproduce.md`.

## mac-mini-m4, 2026-11-10, full

Apple M4 (4P + 6E), 10-core GPU, 16 GB, macOS 27.0.1, 1920x1080 at 60 Hz, AC power. Commit `ae6e33358`. No mob window contended. Client taken through a Screen Sharing session.

Mob-bench, 50,000 pigs, no client:

| phase | mcrs TPS | vanilla TPS | mcrs sprint ms/tick | vanilla sprint ms/tick | mcrs / vanilla RSS MiB |
|---|---:|---:|---:|---:|---:|
| idle | 20.0 | 20.0 | 1.14 | 1.61 | 500 / 5620 |
| roam-still | 20.0 | 4.18 | 4.75 | 238 | 600 / 6596 |
| roam-stir | 20.0 | 3.41 | 8.77 | 284 | 645 / 6798 |
| crowd-still | 20.0 | 2.29 | 4.16 | 417 | 757 / 7569 |
| crowd-stir | 20.0 | 2.09 | 13.4 | 479 | 755 / 7762 |
| stimulus | 20.0 | 1.99 | 13.2 | 489 | 755 / 7902 |
| cost-still | 20.0 | 7.99 | 1.74 | 125 | 785 / 8548 |
| cost-stir | 20.0 | 7.62 | 1.73 | 126 | 784 / 8552 |

With the watcher client: mcrs 19.99 to 20.0 TPS in every phase; vanilla 1.46 to 3.45 TPS in the roam, crowd and stimulus phases and 6.1 to 6.3 in the cost phases.

Client, fullscreen 1920x1080, Metal, cold / `MCRS_HOT=1`:

| scenario | fps | wall frame median / p99 ms |
|---|---:|---:|
| floor | 1163 / 1162 | 0.80 / 1.73 |
| base | 324 / 329 | 3.08 / 3.60 |
| flight | 309 / 312 | 3.22 / 3.78 |

Server start to `entering Playing state`: 1.23 s (registries 391 ms). Worldgen: `overworld_pipeline 64 777` 3.29 ms a column, `64 845` 3.16 ms, `beta_chunks 32 1 12345` 2477 chunks/s.
