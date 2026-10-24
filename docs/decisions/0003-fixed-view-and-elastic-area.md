# 0003 — Same 36×36 view for everyone; open area sized by player count

**Status:** accepted

## Context

The map should feel the same whether 5 or 5000 people are online: usually 2–3 other tanks on
screen, and always a wall or two in sight so movement is visible. The map is fixed, but the
playable part should widen when crowded and narrow when empty — without ever hurting a player.

## Decision

- Every player sees exactly 36×36 cells, on desktop and phone alike, so nobody gets a bigger view.
- The map is 2048×2048 with one wall piece per 18×18 block, which guarantees a wall in every view
  and passages of at least 4 cells.
- The open area targets a centred square of about 400 cells² per player (minimum side 60).
- Its four sides move independently. Growing is immediate; shrinking is slow, needs a 15% margin
  above target, and a side never closes over a tank. New players spawn inside the target, so the
  rim empties on its own.
- Outside the open area is drawn solid, like an infinitely thick wall — no separate border.

## Consequences

- A player camping on an edge only holds that one side open; the rest still shrinks.
- After a crowd leaves, the area takes a while to shrink back; nobody is ever pushed or killed by
  the edge.
