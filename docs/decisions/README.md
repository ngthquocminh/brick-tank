# Decision records

Short records of the choices that shape Brick Tank: the context, what was decided, and what it
costs. Add a new numbered file when a decision changes; mark the old one as superseded rather than
editing history.

| # | Decision |
|---|---|
| [0001](0001-one-shared-world.md) | One persistent world on one server-authoritative process |
| [0002](0002-binary-snapshots-every-tick.md) | Binary WebSocket protocol with per-player snapshots every tick |
| [0003](0003-fixed-view-and-elastic-area.md) | Same 36×36 view for everyone; open area sized by player count |
| [0004](0004-predict-own-tank-only.md) | Predict only your own tank; no lag compensation |
| [0005](0005-spawn-protection-and-afk.md) | 3 s spawn invulnerability and 60 s AFK removal |
| [0006](0006-latch-short-taps.md) | Latch key taps shorter than a tick |
| [0007](0007-player-cap-from-measurement.md) | Player cap of 3000 per 2-vCPU server, from load tests |
| [0008](0008-inputs-per-client-tick.md) | One input per client tick, applied one per server tick |
