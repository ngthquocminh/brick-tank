# 0006 — Latch key taps shorter than a tick

**Status:** accepted

## Context

The server samples held keys once per 50 ms tick. A key pressed and released within one tick
(a quick keyboard tap, a light touch on a phone) arrived as two INPUT messages before the tick ran,
so the tank never turned, moved or fired. Browser testing showed this immediately.

## Decision

- A direction press is kept as a "tap" until the tank next acts (its movement cooldown reaches 0),
  and a fire press is kept for the next tick.
- The client predictor latches presses the same way, so prediction and server agree.

## Consequences

- Every press does something, however short.
- Holding a key behaves exactly as before; a release after a hold does not add an extra step.
