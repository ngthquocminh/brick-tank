# 0008 — One input per client tick, applied one per server tick

**Status:** accepted

## Context

Under [0004](0004-predict-own-tank-only.md) the client sent INPUT only when its keys changed, and
the server applied the latest one on every tick until the next arrived. Snapshots acked
`(ack_seq, ack_ticks)`: "input `seq`, applied for `ack_ticks` ticks". The client matched that to
its own count of local ticks under `seq`.

The two counts come from two unsynchronised 50 ms clocks, one starting when the input was sent
and the other when it arrived. With held keys they agree, but with rapid presses each input lasts
only one to three ticks and the counts are often one apart: the server ran an input for a tick
more, or less, than the client predicted, or acked a tick the client had not run yet. The client
then found no matching history entry, snapped to the server state and discarded its history, so
the acks that followed missed too. In a simulation of rapid random presses, 25–85% of snapshots
missed on localhost depending on clock phase, and nearly all of them from 40 ms of one-way latency
on, with the own tank visibly corrected on 3–22% of snapshots. Players saw it as stutter. The
tap latch of [0006](0006-latch-short-taps.md), run separately on both sides, drifted for the same
reason.

## Decision

- The client sends one INPUT per local tick, numbered, carrying what it applied on that tick:
  the direction (taps already latched, in the client only) and fire.
- The server queues INPUTs per client and applies exactly one per server tick, in order. The
  snapshot acks the client tick it applied. With none queued, the tank waits: no move, no
  cooldown, no fire, so applying the late INPUT later still matches the client.
- The queue is capped at 4. On overflow, or when 3 or more have waited at every tick for 40
  ticks, the oldest is dropped, and its fire press carries over. The server never applies two
  client ticks in one, so sending faster cannot make a tank faster.
- The client rebases on the server state at `ack` and replays its later ticks. A missing entry
  no longer clears the history.
- AFK counts a change of input as activity, since INPUTs now arrive all the time.

## Consequences

- The own tank is only corrected when the world disagrees with the prediction (another tank in
  the way, the area edge, a dropped INPUT), not by timing. The simulation shows no corrections at
  any latency or jitter tried.
- Each client sends 20 INPUTs a second (4 bytes each). The rate limit is now 100 INPUTs a second,
  leaving room for a burst after a stall; capacity from [0007](0007-player-cap-from-measurement.md)
  should be re-measured with the updated load-test bots.
- Other players see a tank with a jittery connection pause for a tick when its INPUT is late, and
  up to a few ticks of extra delay while a backlog drains.
- Within a tick, the last press still wins. Buffering several quick presses is a separate,
  client-only change.
