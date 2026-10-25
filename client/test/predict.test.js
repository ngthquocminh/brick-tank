import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Predictor } from '../js/predict.js';
import { DIR, stepMove } from '../js/rules.js';

const free = () => false;
const pos = (p) => [p.tank.x, p.tank.y];

function walkUp(ticks, from = 0) {
  const p = new Predictor();
  p.reset({ x: 10, y: 10, dir: DIR.UP, cd: 0 });
  p.tick = from;
  for (let i = 0; i < ticks; i++) p.localTick(DIR.UP, false, true, free, free);
  return p;
}

test('predicts immediately and reports the tick', () => {
  const p = new Predictor();
  p.reset({ x: 10, y: 10, dir: DIR.UP, cd: 0 });
  assert.deepEqual(p.localTick(DIR.UP, true, true, free, free), { tick: 1, held: DIR.UP, fire: true });
  assert.deepEqual(pos(p), [10, 9]);
});

test('reconcile replays unacked ticks', () => {
  const p = walkUp(4);
  assert.deepEqual(pos(p), [10, 8]);
  p.reconcile({ x: 10, y: 9, dir: DIR.UP, cd: 1 }, 1, false, free);
  assert.deepEqual(pos(p), [10, 8]);
  assert.equal(p.history.length, 3);
});

test('server correction wins', () => {
  const p = walkUp(4);
  p.reconcile({ x: 10, y: 10, dir: DIR.UP, cd: 1 }, 1, false, free);
  assert.deepEqual(pos(p), [10, 9]);
});

test('no ack yet replays every local tick', () => {
  const p = walkUp(4);
  p.reconcile({ x: 10, y: 10, dir: DIR.UP, cd: 0 }, null, false, free);
  assert.deepEqual(pos(p), [10, 8]);
  assert.equal(p.history.length, 4);
});

test('ack across the 16-bit wrap', () => {
  const p = walkUp(4, 65533);
  assert.deepEqual(p.history.map((e) => e.tick), [65534, 65535, 0, 1]);
  p.reconcile({ x: 10, y: 9, dir: DIR.UP, cd: 0 }, 65535, false, free);
  assert.deepEqual(p.history.map((e) => e.tick), [0, 1]);
  assert.deepEqual(pos(p), [10, 8]);
});

test('ack of a trimmed tick keeps the newer ones', () => {
  const p = walkUp(300);
  p.reconcile({ x: 10, y: 10, dir: DIR.UP, cd: 0 }, 50, false, free);
  assert.equal(p.history.length, 200);
});

test('own bullet predicted and cleared', () => {
  const p = new Predictor();
  p.reset({ x: 10, y: 10, dir: DIR.UP, cd: 0 });
  p.localTick(null, true, true, free, free);
  assert.deepEqual([p.bullet.x, p.bullet.y], [11, 9]);
  p.localTick(null, true, true, free, free);
  assert.deepEqual([p.bullet.x, p.bullet.y], [11, 8]);
  p.reconcile({ x: 10, y: 10, dir: DIR.UP, cd: 0 }, 2, false, free);
  assert.equal(p.bullet, null);
});

// Server order (World::step): fire is checked against the bullet the tank had at the start of the
// tick, then bullets move. So a tank cannot refire on the tick its old bullet ends.
test('holding fire refires only the tick after the old bullet ends', () => {
  const p = new Predictor();
  p.reset({ x: 10, y: 10, dir: DIR.UP, cd: 0 });
  const spawned = [];
  for (let t = 1; t <= 28; t++) {
    const had = p.bullet;
    p.localTick(null, true, true, free, free);
    if (p.bullet && p.bullet !== had) spawned.push(t);
  }
  // Fired on tick 1, last moves on tick 26 (25 cells), so the next shot is on tick 27.
  assert.deepEqual(spawned, [1, 27]);
});

test('no refire on the tick the old bullet hits a wall', () => {
  const p = new Predictor();
  p.reset({ x: 10, y: 10, dir: DIR.UP, cd: 0 });
  const wall = (x, y) => y <= 6;
  p.localTick(null, true, true, free, wall);
  assert.deepEqual([p.bullet.x, p.bullet.y], [11, 9]);
  p.localTick(null, true, true, free, wall);
  p.localTick(null, true, true, free, wall);
  p.localTick(null, true, true, free, wall);
  assert.equal(p.bullet, null, 'hit the wall on tick 4');
  p.localTick(null, true, true, free, wall);
  assert.deepEqual([p.bullet.x, p.bullet.y], [11, 9], 'refired on tick 5');
});

test('history is capped while no snapshots arrive', () => {
  const p = walkUp(1000);
  assert.ok(p.history.length <= 200);
});

test('tap shorter than a tick still turns and fires, and is reported', () => {
  const p = new Predictor();
  p.reset({ x: 10, y: 10, dir: DIR.UP, cd: 0 });
  p.press(DIR.RIGHT, true);
  assert.deepEqual(p.localTick(null, false, true, free, free), { tick: 1, held: DIR.RIGHT, fire: true });
  assert.equal(p.tank.dir, DIR.RIGHT);
  assert.ok(p.bullet);
  assert.deepEqual(p.localTick(null, false, true, free, free), { tick: 2, held: null, fire: false });
  p.localTick(null, false, true, free, free);
  assert.deepEqual(pos(p), [10, 10]);
});

test('tap during cooldown is reported until the tank acts', () => {
  const p = new Predictor();
  p.reset({ x: 10, y: 10, dir: DIR.RIGHT, cd: 1 });
  p.press(DIR.RIGHT, false);
  assert.equal(p.localTick(null, false, true, free, free).held, DIR.RIGHT);
  assert.equal(p.localTick(null, false, true, free, free).held, DIR.RIGHT);
  assert.equal(p.localTick(null, false, true, free, free).held, null);
  assert.deepEqual(pos(p), [11, 10]);
});

// End to end against a model of the server's input queue (server/src/game.rs): one report per
// server tick, a held tick when none has arrived. Two unsynchronised 50 ms clocks, latency and
// jitter, rapid random presses. The own tank must never be corrected.
test('rapid presses under latency and jitter cause no corrections', () => {
  for (const [oneWay, jitter, phase] of [[0, 0, 10], [0, 0, 40], [1, 2, 25], [40, 15, 25], [80, 30, 7]]) {
    let seed = 12345;
    const rnd = () => (seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648;
    const events = [];
    let order = 0;
    const at = (t, fn) => events.push([t, order++, fn]);

    const start = { x: 1000, y: 1000, dir: DIR.UP, cd: 0 };
    const server = { tank: { ...start }, queue: [], ack: null, arrivedBy: 0, snapBy: 0 };
    const p = new Predictor();
    p.reset(start);
    let held = null, corrections = 0;

    for (let t = 100; t < 20000;) {
      const dir = Math.floor(rnd() * 4), hold = 30 + rnd() * 170;
      at(t, () => { held = dir; p.press(dir, false); });
      at(t + hold, () => { held = null; });
      t += hold + 10 + rnd() * 70;
    }
    for (let n = 0; n * 50 < 21000; n++) {
      at(n * 50, () => {
        const r = p.localTick(held, false, true, free, free);
        // TCP keeps order: a report never overtakes the one before it.
        const arrive = Math.max(server.arrivedBy, n * 50 + oneWay + rnd() * jitter);
        server.arrivedBy = arrive;
        at(arrive, () => server.queue.push(r));
      });
      at(phase + n * 50, () => {
        const r = server.queue.shift();
        if (r) {
          stepMove(server.tank, r.held, free);
          server.ack = r.tick;
        }
        const snap = { own: { ...server.tank }, ack: server.ack };
        server.snapBy = Math.max(server.snapBy, phase + n * 50 + oneWay + rnd() * jitter);
        at(server.snapBy, () => {
          const before = { ...p.tank };
          p.reconcile(snap.own, snap.ack, false, free);
          if (JSON.stringify(before) !== JSON.stringify(p.tank)) corrections++;
        });
      });
    }
    while (events.length) {
      events.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
      events.shift()[2]();
    }
    assert.equal(corrections, 0, `one-way ${oneWay} ms, jitter ${jitter} ms, phase ${phase} ms`);
  }
});
