# Wire protocol

All game traffic is binary WebSocket frames on `/ws`, little-endian. The first byte of every
message is its type. The Rust implementation is `server/src/protocol.rs`; the browser one is
`client/js/protocol.js`. Both must change together.

## Conventions

- `str` = `u8 len` followed by `len` ASCII bytes.
- Direction codes: `0` up, `1` right, `2` down, `3` left.
- Held-input code: `0` none, `1`–`4` = direction + 1.
- Player names: 1–10 characters from `A–Z 0–9 _ - .`. Anything else closes the connection.

## Client → server

| Type | Name | Payload |
|---|---|---|
| `0x01` | JOIN | `str name` — spawn a tank (ignored while you already have one) |
| `0x02` | INPUT | `u16 seq`, `u8 bits` — bits 0–2 held-input code, bit 3 fire held |

Send INPUT only when the held state changes, incrementing `seq` each time. INPUT before JOIN is
ignored.

## Server → client

| Type | Name | Payload |
|---|---|---|
| `0x10` | WELCOME | `u32 tank_id`, `u32 map_version` |
| `0x11` | SNAPSHOT | see below (sent every tick, latest wins) |
| `0x12` | NAMES | `u8 n`, then n × (`u32 id`, `str name`) |
| `0x13` | BOARD | see below (every 20 ticks) |
| `0x14` | DEAD | `u8 cause` (0 shot, 1 own bullet, 2 AFK), `str killer` (empty if none), `u16 kills`, `u32 secs`, `u8 record_flags` (bit 0 kills record, bit 1 time record) |
| `0x15` | FULL | no payload — the server is at its player cap |

### SNAPSHOT (`0x11`)

| Field | Type | Notes |
|---|---|---|
| tick | `u32` | |
| own_x, own_y | `u16`, `u16` | top-left of your 3×3 tank |
| own_flags | `u8` | bits 0–1 direction, bit 2 invulnerable, bit 3 AFK warning, bit 4 has a bullet in flight |
| own_cd | `u8` | movement cooldown (0 = can act this tick) |
| own_kills | `u16` | |
| ack_seq | `u16` | last INPUT `seq` the server applied (0 before any input) |
| ack_ticks | `u16` | ticks that input has been applied, including this one |
| l, t, r, b | 4 × `u16` | open area, half-open: `l <= x < r`, `t <= y < b` |
| n_tanks | `u8` | then n × (`u32 id`, `u8 dx`, `u8 dy`, `u8 flags` (bits 0–1 direction, bit 2 invulnerable), `u16 kills`) |
| n_bullets | `u8` | then n × (`u8 dx`, `u8 dy`, `u8 flags` (bit 0 = your bullet)) |

The fixed part is 27 bytes; each tank adds 9 and each bullet 3. Lists are capped at 255 entries.

**Positions.** The server's view origin is `vx = own_x − 17`, `vy = own_y − 17` (your tank sits in
the middle of a 36×36 view). Other tanks and bullets within 4 cells of the view are included, with
`dx = x − vx + 8` and `dy = y − vy + 8`, so `x = own_x − 25 + dx`. Your own tank is never in the
tank list.

**Acks.** The client counts, per `seq`, how many local ticks it has run under that input (from 1).
The server state in a snapshot corresponds to the local tick where that count equals
`ack_ticks`; the client rebases its tank there and replays the later ticks.

### BOARD (`0x13`)

```
u16 online
u8 n, n × (str name, u16 kills)    living players by kills (top 20)
u8 n, n × (str name, u32 secs)     living players by time alive (top 20)
u8 n, n × (str name, u16 kills)    all-time kills records (top 5)
u8 n, n × (str name, u32 secs)     all-time time-alive records (top 5)
u16 my_rank_kills                  0 when you are not alive
u16 my_rank_time
```

## Limits

- At most 5 WebSocket connections per IP (configurable).
- Frames over 64 bytes, Text frames, or malformed messages close the connection.
- More than 120 messages per second on one connection closes it; more than 60 INPUTs within 20
  ticks drops the player.

## `/map.bin`

The fixed wall map, fetched once over HTTP and cached as immutable:

```
"BTMP"          4 bytes magic
u8   version    1
u16  map_size   2048
u32  count
count × (u16 x, u16 y, u8 w, u8 h)    wall rectangles
```

`map_version` in WELCOME is the FNV-1a 32-bit hash of this file.
