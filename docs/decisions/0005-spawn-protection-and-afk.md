# 0005 — 3 s spawn invulnerability and 60 s AFK removal

**Status:** accepted; since [0008](0008-inputs-per-client-tick.md) "no input" means input that
has not changed, as INPUTs arrive every tick

## Context

At the target density there is often another tank within 20 cells of any spawn point, and a
bullet crosses that in under a second, so "one hit kills" alone would let people die on arrival.
Separately, idle tanks (or phones left with the tab open) would farm the time-alive board and keep
the map large.

## Decision

- **Spawn protection:** for 60 ticks (3 s) a new tank blinks, can move but cannot fire, and
  bullets pass through it.
- **AFK:** a tank that sends no input for 1200 ticks (60 s) is removed as a death, and its life
  still counts for records. From tick 1000 the screen flashes "AFK?". Sitting still is fine as
  long as the player touches a key now and then.

## Consequences

- "One hit kills" has exactly one exception: the first 3 seconds.
- The time-alive board reflects players who are actually there.
