import { test } from 'node:test';
import assert from 'node:assert/strict';
import { encodeInput, encodeJoin, decode } from '../js/protocol.js';

test('encodeInput golden bytes', () => {
  assert.deepEqual([...encodeInput(0x1234, 2, true)], [2, 0x34, 0x12, 0x0a]);
});

test('encodeJoin golden bytes', () => {
  assert.deepEqual([...encodeJoin('LAN')], [1, 3, 76, 65, 78]);
});

test('snapshot decodes to absolute positions', () => {
  const b = [0x11, 1, 0, 0, 0, 100, 0, 200, 0, 0b10101, 1, 7, 0, 9, 0, 3,
    0x84, 3, 0x85, 3, 0xe8, 3, 0xe9, 3,
    1, 42, 0, 0, 0, 10, 20, 2, 5, 0,
    1, 9, 8, 1];
  const m = decode(new Uint8Array(b).buffer);
  assert.equal(m.type, 'snapshot');
  assert.equal(m.tick, 1);
  assert.deepEqual([m.ownX, m.ownY, m.ownDir, m.ownInvuln, m.ownAfkWarn, m.ownHasBullet], [100, 200, 1, true, false, true]);
  assert.deepEqual([m.ownCd, m.ownKills, m.ackSeq, m.ackTicks], [1, 7, 9, 3]);
  assert.deepEqual(m.bounds, { l: 900, t: 901, r: 1000, b: 1001 });
  assert.deepEqual(m.tanks, [{ id: 42, x: 85, y: 195, dir: 2, invuln: false, kills: 5 }]);
  assert.deepEqual(m.bullets, [{ x: 84, y: 183, own: true }]);
});

test('board, dead, names, welcome, full decode', () => {
  const str = (s) => [s.length, ...[...s].map((c) => c.charCodeAt(0))];
  const board = [0x13, 42, 0, 1, ...str('LAN'), 12, 0, 1, ...str('H'), 84, 1, 0, 0, 0, 1, ...str('Y'), 1, 0, 0, 0, 3, 0, 0, 0];
  assert.deepEqual(decode(new Uint8Array(board).buffer), {
    type: 'board', online: 42,
    aliveKills: [['LAN', 12]], aliveTime: [['H', 340]], recKills: [], recTime: [['Y', 1]],
    myRankKills: 3, myRankTime: 0,
  });
  const dead = [0x14, 0, ...str('LAN'), 4, 0, 61, 0, 0, 0, 2];
  assert.deepEqual(decode(new Uint8Array(dead).buffer), { type: 'dead', cause: 0, killer: 'LAN', kills: 4, secs: 61, recordFlags: 2 });
  assert.deepEqual(decode(new Uint8Array([0x12, 1, 5, 0, 0, 0, ...str('A')]).buffer), { type: 'names', names: [[5, 'A']] });
  assert.deepEqual(decode(new Uint8Array([0x10, 5, 0, 0, 0, 0xef, 0xbe, 0xad, 0xde]).buffer), { type: 'welcome', tankId: 5, mapVersion: 0xdeadbeef });
  assert.deepEqual(decode(new Uint8Array([0x15]).buffer), { type: 'full' });
});
