// Binary wire protocol (little-endian), mirroring server/src/protocol.rs.
import { VIEW_HALF, DX_OFFSET } from './rules.js';

export function encodeJoin(name) {
  const out = new Uint8Array(2 + name.length);
  out[0] = 0x01;
  out[1] = name.length;
  for (let i = 0; i < name.length; i++) out[2 + i] = name.charCodeAt(i);
  return out;
}

export function encodeInput(seq, heldCode, fire) {
  return new Uint8Array([0x02, seq & 0xff, (seq >> 8) & 0xff, (heldCode & 7) | (fire ? 8 : 0)]);
}

export function decode(buf) {
  const v = new DataView(buf);
  let i = 0;
  const u8 = () => v.getUint8(i++);
  const u16 = () => { const n = v.getUint16(i, true); i += 2; return n; };
  const u32 = () => { const n = v.getUint32(i, true); i += 4; return n; };
  const str = () => { const n = u8(); let s = ''; for (let k = 0; k < n; k++) s += String.fromCharCode(u8()); return s; };
  const list = (num) => { const n = u8(), a = []; for (let k = 0; k < n; k++) a.push([str(), num()]); return a; };

  switch (u8()) {
    case 0x10: return { type: 'welcome', tankId: u32(), mapVersion: u32() };
    case 0x11: {
      const m = { type: 'snapshot', tick: u32(), ownX: u16(), ownY: u16() };
      const f = u8();
      Object.assign(m, { ownDir: f & 3, ownInvuln: !!(f & 4), ownAfkWarn: !!(f & 8), ownHasBullet: !!(f & 16) });
      Object.assign(m, { ownCd: u8(), ownKills: u16(), ackSeq: u16(), ackTicks: u8() });
      m.bounds = { l: u16(), t: u16(), r: u16(), b: u16() };
      const ox = m.ownX - VIEW_HALF - DX_OFFSET, oy = m.ownY - VIEW_HALF - DX_OFFSET;
      m.tanks = [];
      for (let n = u8(); n > 0; n--) {
        const id = u32(), x = ox + u8(), y = oy + u8(), tf = u8();
        m.tanks.push({ id, x, y, dir: tf & 3, invuln: !!(tf & 4), kills: u16() });
      }
      m.bullets = [];
      for (let n = u8(); n > 0; n--) {
        const x = ox + u8(), y = oy + u8();
        m.bullets.push({ x, y, own: !!(u8() & 1) });
      }
      return m;
    }
    case 0x12: {
      const names = [];
      for (let n = u8(); n > 0; n--) names.push([u32(), str()]);
      return { type: 'names', names };
    }
    case 0x13: return {
      type: 'board', online: u16(), aliveKills: list(u16), aliveTime: list(u32),
      recKills: list(u16), recTime: list(u32), myRankKills: u16(), myRankTime: u16(),
    };
    case 0x14: return { type: 'dead', cause: u8(), killer: str(), kills: u16(), secs: u32(), recordFlags: u8() };
    case 0x15: return { type: 'full' };
    default: return { type: 'unknown' };
  }
}
