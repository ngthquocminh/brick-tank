// Local prediction of the player's own tank and bullet, reconciled against server acks.
import { stepMove, muzzle, DELTA, BULLET_RANGE } from './rules.js';

const HISTORY_MAX = 200;

// Whether client tick `a` comes after `b` (tick numbers wrap at 16 bits).
const after = (a, b) => {
  const d = (a - b) & 0xffff;
  return d !== 0 && d < 0x8000;
};

export class Predictor {
  constructor() {
    this.tank = { x: 0, y: 0, dir: 0, cd: 0 };
    this.bullet = null;
    this.history = [];
    this.tick = 0;
    this.tap = null;
    this.fireTap = false;
  }

  // Latches a press so a tap shorter than one tick still acts.
  press(held, fire) {
    if (held !== null) this.tap = held;
    if (fire) this.fireTap = true;
  }

  reset(own) {
    this.tank = { ...own };
    this.bullet = null;
    this.history = [];
  }

  // Runs the next client tick and returns its report for the server: { tick, held, fire }.
  localTick(held, fire, canFire, blocked, bulletBlocked) {
    held = held ?? this.tap;
    if (this.tank.cd === 0) this.tap = null;
    fire = fire || this.fireTap;
    this.fireTap = false;
    this.tick = (this.tick + 1) & 0xffff;
    const entry = { tick: this.tick, held, fired: false };
    stepMove(this.tank, held, blocked);
    // Like the server: a bullet that ends this tick still blocks firing until the next one.
    const b = this.bullet;
    if (b) {
      b.x += DELTA[b.dir][0];
      b.y += DELTA[b.dir][1];
      if (--b.range <= 0 || bulletBlocked(b.x, b.y)) this.bullet = null;
    }
    if (fire && canFire && !b) {
      const [x, y] = muzzle(this.tank.x, this.tank.y, this.tank.dir);
      if (!bulletBlocked(x, y)) this.bullet = { x, y, dir: this.tank.dir, range: BULLET_RANGE };
      entry.fired = true;
    }
    this.history.push(entry);
    if (this.history.length > HISTORY_MAX) this.history.shift();
    return { tick: this.tick, held, fire };
  }

  // Rebases on the server's state after client tick `ack` (null: none applied yet) and replays
  // the local ticks the server has not applied.
  reconcile(own, ack, ownHasBullet, blocked) {
    this.tank = { ...own };
    const later = ack === null ? this.history : this.history.filter((e) => after(e.tick, ack));
    for (const e of later) stepMove(this.tank, e.held, blocked);
    this.history = later;
    if (!ownHasBullet && !later.some((e) => e.fired)) this.bullet = null;
  }
}
