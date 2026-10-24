# Budgets and measurements

Targets from `docs/spec.md` sections 9 and 14. Each has a script; the numbers below are the last recorded run.

| Budget                                | Target    | Measured      | Check                                   |
| ------------------------------------- | --------- | ------------- | --------------------------------------- |
| Portal initial HTML + CSS + JS, gzip  | ≤ 150 KiB | 85.9 KiB      | `scripts/web-budget` (in `scripts/ci`)  |
| Idle RSS with 1,000 Grants            | ≤ 64 MiB  | 8.3 MiB       | `scripts/bench`                         |
| p95 `/verify` at 500 checks/s         | < 5 ms    | 0.3 to 0.7 ms | `scripts/bench`                         |
| Image size, linux/amd64, uncompressed | ≤ 20 MiB  | 13.7 MiB      | `scripts/image-size` (in `release.yml`) |

## Portal budget

`scripts/web-budget` builds the Portal and gzips (level 9) the prerendered page, its stylesheets and every script reachable from it through preloads and static imports. Lazy chunks and images do not count. It fails above 150 KiB.

## Runtime: `scripts/bench`

Runs the release binary (`scripts/cargo build --release -p server`) with `profile: production`, SQLite, one gated host, and itself standing in for Traefik (loopback peer, proxy token, `X-Forwarded-*`).

1. Starts `accit` pinned to two CPUs (`taskset -c 0,1`, `BENCH_CPUS` to change).
2. Creates 1,000 Grants through `POST /api/unlock` from 1,000 distinct public Addresses.
3. Waits 5 s, then reads `VmRSS` from `/proc`: the idle figure.
4. Sends `GET /verify` for granted Addresses on the gated host, open loop at 500 per second for 30 s (`BENCH_SECONDS`), over keep-alive connections as Traefik does. Every reply must be 204.
5. Reports nearest-rank p50, p95 and p99, and exits non-zero when RSS or p95 misses its target.

Last run: Intel Core i5-14500 host, Linux 5.15, server on 2 pinned CPUs, load generator (bun) on the same host over loopback, so the latency excludes the network and Traefik's own hop.

```
grants            1000
idle RSS          8.3 MiB (target <= 64)
peak RSS          8.3 MiB
verify 500/s x 30s  15000 checks
latency p50/p95/p99  0.22 / 0.45 / 3.15 ms (p95 target < 5)
```

Across runs p95 stayed between 0.3 and 0.7 ms; p99 swung from 3 to 6 ms, probably from the shared load generator; it is not a target. The first 2 s of each run are discarded as warm-up. A real 2-vCPU host should be measured again before relying on the margin.

## Image

Built on this host with `docker build -f docker/single/Dockerfile -t accit:budget .` (linux/amd64): 14.4 MB (13.7 MiB) uncompressed, of which the binary is 7.7 MB. The arm64 image is built only by `release.yml` (QEMU); it was not built or measured on this host.
