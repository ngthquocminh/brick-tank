import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseMap } from '../js/map.js';

test('parseMap reads rects into a bitset', () => {
  const b = new Uint8Array([66, 84, 77, 80, 1, 0, 8, 1, 0, 0, 0, 5, 0, 6, 0, 2, 1]);
  const m = parseMap(b.buffer);
  assert.equal(m.size, 2048);
  assert.equal(m.isWall(5, 6), true);
  assert.equal(m.isWall(6, 6), true);
  assert.equal(m.isWall(7, 6), false);
  assert.equal(m.isWall(5, 7), false);
  assert.equal(m.isWall(-1, 0), true);
  assert.equal(m.isWall(2048, 0), true);
  assert.equal(typeof m.version, 'number');
});
