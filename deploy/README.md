# Deploying Brick Tank

## Run on a VPS

One server holds the whole world, so pick a location with balanced latency for your players
(Singapore for Asia, Frankfurt for Europe/Middle East).

```bash
git clone <repo> brick-tank && cd brick-tank
TRUST_PROXY=1 docker compose up -d --build server   # listens on :8080, records in the "data" volume
```

Put Caddy in front for HTTPS/WSS (`deploy/Caddyfile`, change the domain), either installed on the
host or as another compose service. With a proxy in front, set `TRUST_PROXY=1` so the per-IP limit
uses `X-Forwarded-For`. Then close port 8080 to the outside so only Caddy reaches it.

## Environment variables

| Variable | Default | Meaning |
|---|---|---|
| `BIND` | `0.0.0.0:8080` | listen address |
| `STATIC_DIR` | `client` (`/srv/client` in the image) | client files |
| `RECORDS` | `records.json` (`/data/records.json` in compose) | all-time records file |
| `MAX_PLAYERS` | `3000` | players above this get "SERVER FULL" |
| `MAX_PER_IP` | `5` | concurrent sockets per IP |
| `TRUST_PROXY` | `0` | `1` = take the client IP from `X-Forwarded-For` |

## Capacity (measured 2026-10-24)

Server container capped at 2 CPUs (`cpus: 2`), bots from `loadtest` on the same machine
(16-core host), every bot moving and shooting at random:

| Bots | Tick mean / max | Snapshot gap p50 / p99 | Server CPU | Server RAM | Outgoing |
|---|---|---|---|---|---|
| 1000 | 2.5 / 8 ms | 50 / 55 ms | — | — | ~1.8 MB/s |
| 3000 | 9–10 / 31 ms | 49 / 57–77 ms | 172% | 408 MB | ~5.4 MB/s |
| 4000 | 37 / 86 ms | 150–200 / 260–380 ms | 200% (saturated) | 550 MB | — |
| 5000 | 25 / 76 ms | 100–115 / 160–187 ms | 200% (saturated) | 680 MB | — |

Targets were tick max < 50 ms and snapshot p99 < 100 ms, so `MAX_PLAYERS` defaults to **3000** for
a 2-vCPU box. Re-run the load test on the real VPS before raising it:

```bash
MAX_PER_IP=100000 docker compose up -d server
docker compose run --rm dev cargo run -p loadtest --release -- ws://server:8080/ws 3000 500
docker compose logs -f server   # "tick mean ... max ..." every 10 s
```
