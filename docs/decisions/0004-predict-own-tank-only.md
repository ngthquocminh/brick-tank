# 0004 — Predict only your own tank; no lag compensation

**Status:** accepted

## Context

Players are worldwide but share one server, so some have 150 ms or more of round trip. Without
help, every key press would show up a round trip late.

## Decision

- The client simulates its own tank and its own bullet locally with the same rules as the server,
  so presses show immediately.
- INPUT messages carry a sequence number; snapshots acknowledge `(ack_seq, ack_ticks)`. The client
  rebases on the server's state at that point and replays newer local ticks; if it cannot find
  the point, it snaps to the server.
- Other tanks and bullets are shown exactly where the last snapshot put them.
- The server does not rewind time for hits (no lag compensation).

## Consequences

- Movement feels instant everywhere; a mismatch shows as the own tank hopping a cell.
- Players far from the server see opponents slightly in the past and are at a small disadvantage
  in duels. This is accepted to keep the server simple.
- Movement rules live in two languages; shared test vectors in `shared/movement_cases.json` keep
  them equal.
