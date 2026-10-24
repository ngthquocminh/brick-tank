import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stepMove, muzzle, boxOverlap, DIR } from '../js/rules.js';

const cases = JSON.parse(readFileSync(new URL('../../shared/movement_cases.json', import.meta.url)));

for (const c of cases) {
  test(`shared case: ${c.name}`, () => {
    const [l, t, r, b] = c.bounds;
    const blocked = (x, y) => {
      for (let j = 0; j < 3; j++) for (let i = 0; i < 3; i++) {
        if (c.walls.some(([wx, wy, ww, wh]) => x + i >= wx && x + i < wx + ww && y + j >= wy && y + j < wy + wh)) return true;
      }
      if (x < l || y < t || x + 3 > r || y + 3 > b) return true;
      return c.others.some(([ox, oy]) => boxOverlap(x, y, ox, oy));
    };
    const tank = { x: c.start[0], y: c.start[1], dir: c.start[2], cd: c.start[3] };
    c.inputs.forEach((code, i) => {
      stepMove(tank, code === 0 ? null : code - 1, blocked);
      assert.deepEqual([tank.x, tank.y, tank.dir, tank.cd], c.expect[i], `tick ${i + 1}`);
    });
  });
}

test('muzzle positions', () => {
  assert.deepEqual(muzzle(10, 10, DIR.UP), [11, 9]);
  assert.deepEqual(muzzle(10, 10, DIR.RIGHT), [13, 11]);
  assert.deepEqual(muzzle(10, 10, DIR.DOWN), [11, 13]);
  assert.deepEqual(muzzle(10, 10, DIR.LEFT), [9, 11]);
});
