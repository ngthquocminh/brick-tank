# 0001 — One persistent world on one server-authoritative process

**Status:** accepted

## Context

The game has no matches or rooms: anyone who opens the page drops into the same world, one hit
kills, and dying means starting over. The goal is to keep it as light as possible so a single
server holds as many players as it can.

## Decision

- One Rust process holds the whole world. A single game thread steps it at 20 Hz; Tokio handles
  sockets around it.
- The server is authoritative. Clients send only the keys they hold; the server moves tanks,
  spawns bullets and decides hits.
- No accounts: a player types a name and plays.

## Consequences

- No client can claim a hit or teleport; cheating is limited to what a bot pressing keys can do.
- Players far from the server feel their latency in fights (see 0004).
- Scaling past one machine would need sharding the world, which is out of scope.
- Living state is lost on restart; only the all-time records are persisted.
