# 0002 — Binary WebSocket protocol with per-player snapshots every tick

**Status:** accepted

## Context

Bandwidth and per-message work are what limit how many players one server can carry, more than
the language. Walls never change; tanks and bullets change every tick.

## Decision

- Every message is a compact little-endian binary frame ([protocol](../protocol.md)), never JSON.
- Each player gets their own snapshot every tick, containing only what lies in or near their
  36×36 view, with positions as single bytes relative to the view.
- Walls are not in snapshots: the client downloads `/map.bin` once, cached as immutable.
- Names travel once per viewer (NAMES) the first time a tank enters their view; snapshots carry
  only ids and kill counts.
- The client draws snapshots as they come, without interpolation — the game moves cell by cell.

## Consequences

- A snapshot is typically 30–60 bytes, about 1–2 KB/s per player.
- Snapshots are latest-wins: a slow connection skips frames instead of building a backlog.
- Any change to a message must be made in both `server/src/protocol.rs` and
  `client/js/protocol.js`.
