# 0007 — Player cap of 3000 per 2-vCPU server, from load tests

**Status:** accepted

## Context

The aim was about 5000 players on a 2-vCPU server, but the cap ("server full") had to come from
measurement rather than from the map size, which fits about 10 000.

## Decision

- `MAX_PLAYERS` defaults to 3000. With the server limited to 2 CPUs and bots moving and shooting
  at random, 3000 players kept the tick under 31 ms and snapshot gaps under 77 ms at p99. At 4000
  both CPUs saturate and gaps reach 150–380 ms.
- The cap is an environment variable; measure on the real machine with `loadtest/` before
  raising it. Numbers are in [deploy/README.md](../../deploy/README.md).

## Consequences

- Beyond 3000 players new joiners see "SERVER FULL".
- Raising the ceiling means profiling the socket send path, which dominates at high counts.
