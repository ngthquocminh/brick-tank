import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Predictor } from '../js/predict.js';
import { DIR } from '../js/rules.js';

const free = () => false;
const pos = (p) => [p.tank.x, p.tank.y];

function walkUp(ticks) {
  const p = new Predictor();
  p.reset({ x: 10, y: 10, dir: DIR.UP, cd: 0 });
  p.setSeq(1);
  for (let i = 0; i < ticks; i++) p.localTick(DIR.UP, false, true, free, free);
  return p;
}

test('predicts immediately', () => {
  assert.deepEqual(pos(walkUp(1)), [10, 9]);
});

test('reconcile replays unacked ticks', () => {
  const p = walkUp(4);
  assert.deepEqual(pos(p), [10, 8]);
  p.reconcile({ x: 10, y: 9, dir: DIR.UP, cd: 1 }, 1, 1, false, free);
  assert.deepEqual(pos(p), [10, 8]);
  assert.equal(p.history.length, 3);
});

test('server correction wins', () => {
  const p = walkUp(4);
  p.reconcile({ x: 10, y: 10, dir: DIR.UP, cd: 1 }, 1, 1, false, free);
  assert.deepEqual(pos(p), [10, 9]);
});

test('reconcile snaps when ack entry missing', () => {
  const p = walkUp(4);
  p.reconcile({ x: 3, y: 4, dir: DIR.LEFT, cd: 0 }, 1, 50, false, free);
  assert.deepEqual(p.tank, { x: 3, y: 4, dir: DIR.LEFT, cd: 0 });
  assert.equal(p.history.length, 0);
});

test('own bullet predicted and cleared', () => {
  const p = new Predictor();
  p.reset({ x: 10, y: 10, dir: DIR.UP, cd: 0 });
  p.setSeq(1);
  p.localTick(null, true, true, free, free);
  assert.deepEqual([p.bullet.x, p.bullet.y], [11, 9]);
  p.localTick(null, true, true, free, free);
  assert.deepEqual([p.bullet.x, p.bullet.y], [11, 8]);
  p.reconcile({ x: 10, y: 10, dir: DIR.UP, cd: 0 }, 1, 2, false, free);
  assert.equal(p.bullet, null);
});

test('history is capped while no snapshots arrive', () => {
  const p = walkUp(1000);
  assert.ok(p.history.length <= 200);
});
