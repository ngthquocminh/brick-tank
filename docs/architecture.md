# Architecture

Brick Tank is one Rust process serving one shared world to every browser. The server decides
everything; the browser sends the keys it holds and draws what it is told, predicting only its
own tank so controls feel instant.

```
 browser (client/)                       server (one process)
┌───────────────────┐  WebSocket /ws   ┌──────────────────────────────┐
│ input.js ──INPUT──┼─────────────────►│ net.rs   reader task / conn  │
│ predict.js        │                  │   │ std mpsc (Inbound)       │
│ render.js ◄───────┼──SNAPSHOT/...────│   ▼                          │
│ panel.js          │                  │ game.rs  game thread, 20 Hz  │
└───────────────────┘  HTTP /map.bin   │   │ owns World, Records      │
                       HTTP /*  ──────►│   ▼                          │
                       (static files)  │ world.rs simulation step     │
                                       └──────────────────────────────┘
```

Wire formats are in [protocol.md](protocol.md). The reasons behind the main choices are in
[decisions/](decisions/README.md).

## Server (`server/src`)

| Module | Role |
|---|---|
| `config.rs` | Every tunable constant (tick rate, ranges, timers, sizes). |
| `map.rs` | Generates the fixed wall map from a seed and encodes `/map.bin`. |
| `rules.rs` | Movement rules shared with the client (`step_move`, `muzzle`, `box_overlap`). |
| `area.rs` | The open play area and how it grows and shrinks. |
| `grid.rs` | Spatial hash (16×16-cell buckets) for collision and view queries. |
| `world.rs` | Tanks, bullets, spawning and one simulation tick. |
| `board.rs` | Living leaderboards and the persisted all-time records. |
| `protocol.rs` | Binary encode/decode for every message. |
| `game.rs` | The game thread: clients, inbound messages, ticking, snapshots, boards. |
| `net.rs` | axum router: static files, `/map.bin`, `/ws`, per-IP and rate limits. |
| `main.rs` | Reads environment variables, starts the game thread and the HTTP server. |

### Threads and back-pressure

- **One game thread** (`game::run`) owns the `World`. Every 50 ms it drains the inbound channel,
  runs `Game::tick`, and sleeps until the next deadline.
- **Tokio tasks** handle sockets. Each connection has a reader (decode, rate-limit, forward to the
  game thread) and a writer.
- **Outbound never blocks the game.** Each client has a `watch` channel for snapshots (only the
  latest is kept, so a slow client simply skips frames) and a bounded queue of 64 for reliable
  messages (WELCOME, NAMES, BOARD, DEAD, FULL). If that queue is full the client is dropped.
- **Snapshots are built in parallel** with rayon over an immutable borrow of the world, right
  after the simulation step.

### One tick (`World::step`)

1. Advance the tick counter; remove tanks with no input for 1200 ticks (AFK).
2. Move tanks in two phases: each tank computes where it wants to go against walls and the open
   area, then any move whose box would overlap another tank's current or wanted box is cancelled.
   Two tanks heading into the same cells both stay put.
3. Fire: a tank holding fire (or that tapped fire since the last tick), with no bullet in flight
   and not invulnerable, spawns a bullet in front of its barrel.
4. Bullets move one cell, then are resolved in order: walls and the area edge; bullets on the same
   cell or passing through each other (both vanish); hits on non-invulnerable tanks; range.
5. Kill credit goes to the shooter if it is still alive, even if it dies in the same tick. Hitting
   yourself gives no credit.
6. Update the open area, drop bullets in closed cells, rebuild the spatial hashes.

### The world

- **Map.** 2048×2048 cells from a fixed seed. Each 18×18 block holds exactly one wall piece (a
  2-thick bar of 4–10 cells, or an L) kept 2 cells from the block edges, so gaps are always at
  least 4 cells (a tank is 3) and no area is ever sealed off. Any 36×36 view contains at least one
  whole block, so a wall is always on screen.
- **Open area.** A rectangle with four independent sides. Its target is a square centred on the
  map with side `ceil(sqrt(max(60², players × 400)))`, rounded up to even — about 2–3 other tanks
  per screen at any population. Sides grow one cell per tick. They shrink one cell at most every
  40 ticks, only while the area is more than 15% above target, and a side never moves past a tank,
  so nobody is pushed or killed by the edge. Everything outside is solid.
- **Spawning.** 16 random spots inside the open area and the target; the one farthest from tanks
  and bullets wins. If none is free, a full scan from a random start finds any free spot. A new
  tank is invulnerable for 60 ticks (3 s): it blinks, cannot fire, and bullets pass through it.
- **Taps.** A press is latched until the tank next acts, so a key tapped and released between two
  ticks still turns, moves or fires.

### Boards and records

Every 20 ticks the server ranks living tanks by kills and by time alive and sends each client the
top 20 plus its own rank. When a tank dies (shot, AFK, or disconnect) its life is offered to the
top-5 records, which are saved to a JSON file (write to a temporary file, then rename). A missing
or corrupt file starts empty.

## Client (`client/`)

Plain ES modules, no framework and no build step.

| Module | Role |
|---|---|
| `js/rules.js` | Constants and movement rules mirroring `server/src/rules.rs`. |
| `js/protocol.js` | Message encoding and decoding. |
| `js/map.js` | Parses `/map.bin` into a wall bitset. |
| `js/predict.js` | Local prediction of the player's own tank and bullet. |
| `js/render.js` | The 36×36 LCD: brick sprites, name labels, overlay text. |
| `js/font.js` | 3×5 bitmap font. |
| `js/panel.js` | Score column and rotating leaderboards. |
| `js/input.js` | Keyboard and multi-touch buttons. |
| `js/main.js` | Screens, connection, local tick loop, frame composition, layout. |

### Prediction

The client runs its own 50 ms tick. Each tick it applies the held input to its predicted tank and
bullet and records `(seq, k, held)`, where `k` counts ticks under the current input `seq`. When a
snapshot arrives with `(ack_seq, ack_ticks)`, the client finds the matching history entry, resets
its tank to the server's, and replays the later entries. If the entry is missing (for example
after the tab was in the background) it snaps to the server state. Other tanks and bullets are
drawn exactly as the last snapshot shows them; who got hit is always the server's call.

### Keeping both sides in step

Movement rules exist twice, in Rust and in JavaScript. `shared/movement_cases.json` holds test
vectors that both test suites run, and the constants in `server/src/config.rs` and
`client/js/rules.js` must match.

## Tests and tools

- `docker compose run --rm dev cargo test --workspace` — simulation, protocol, game loop and a
  WebSocket smoke test.
- `npm test` — client rules, protocol, map parsing and prediction.
- `loadtest/` — spawns bots that move and shoot at random and reports throughput and snapshot
  gaps; see [deploy/README.md](../deploy/README.md) for measured capacity.
