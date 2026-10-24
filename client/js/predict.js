// Local prediction of the player's own tank and bullet, reconciled against server acks.
import { stepMove, muzzle, DELTA, BULLET_RANGE } from './rules.js';

const HISTORY_MAX = 200;

export class Predictor {
  constructor() {
    this.tank = { x: 0, y: 0, dir: 0, cd: 0 };
    this.bullet = null;
    this.history = [];
    this.seq = 0;
    this.k = 0;
    this.tap = null;
    this.fireTap = false;
  }

  // Latches a press so a tap shorter than one tick still acts (mirrors the server's tap).
  press(held, fire) {
    if (held !== null) this.tap = held;
    if (fire) this.fireTap = true;
  }

  reset(own) {
    this.tank = { ...own };
    this.bullet = null;
    this.history = [];
    this.seq = 0;
    this.k = 0;
  }

  // Called when a new INPUT is sent; ticks under it are counted from 1.
  setSeq(seq) {
    this.seq = seq;
    this.k = 0;
  }

  localTick(held, fire, canFire, blocked, bulletBlocked) {
    held = held ?? this.tap;
    if (this.tank.cd === 0) this.tap = null;
    fire = fire || this.fireTap;
    this.fireTap = false;
    const entry = { seq: this.seq, k: ++this.k, held, fired: false };
    stepMove(this.tank, held, blocked);
    const b = this.bullet;
    if (b) {
      b.x += DELTA[b.dir][0];
      b.y += DELTA[b.dir][1];
      if (--b.range <= 0 || bulletBlocked(b.x, b.y)) this.bullet = null;
    }
    if (fire && canFire && !this.bullet) {
      const [x, y] = muzzle(this.tank.x, this.tank.y, this.tank.dir);
      if (!bulletBlocked(x, y)) this.bullet = { x, y, dir: this.tank.dir, range: BULLET_RANGE };
      entry.fired = true;
    }
    this.history.push(entry);
    if (this.history.length > HISTORY_MAX) this.history.shift();
  }

  // Rebases on the server's state for (ackSeq, ackTicks) and replays later local ticks.
  reconcile(own, ackSeq, ackTicks, ownHasBullet, blocked) {
    const i = this.history.findIndex((e) => e.seq === ackSeq && e.k === ackTicks);
    this.tank = { ...own };
    if (i < 0) {
      this.history = [];
      if (!ownHasBullet) this.bullet = null;
      return;
    }
    const later = this.history.slice(i + 1);
    for (const e of later) stepMove(this.tank, e.held, blocked);
    this.history = later;
    if (!ownHasBullet && !later.some((e) => e.fired)) this.bullet = null;
  }
}
