// Game flow: name screen → WebSocket → predict own tank, render snapshots, rejoin after death.
import { VIEW, VIEW_HALF, TICK_MS, TANK_SPRITES, boxOverlap } from './rules.js';
import { encodeJoin, encodeInput, decode } from './protocol.js';
import { parseMap } from './map.js';
import { Predictor } from './predict.js';
import { createLcd } from './render.js';
import { createPanel, clock } from './panel.js';
import { createInput } from './input.js';
import { plotText, textWidth } from './font.js';

const $ = (s) => document.querySelector(s);
const lcd = createLcd($('#lcd'));
const panel = createPanel($('#panel'));
const startForm = $('#start'), nameInput = $('#name');
const predictor = new Predictor();
const names = new Map();
const frame = new Uint8Array(VIEW * VIEW);

// state: start | connecting | joining | alive | dead | full | closed
let state = 'start', map = null, ws = null, snap = null, board = null, dead = null;
let name = '', seq = 0, held = 0, fire = false, aliveSince = 0, awaitingFirst = false;

const store = {
  get(k) { try { return localStorage.getItem(k); } catch { return null; } },
  set(k, v) { try { localStorage.setItem(k, v); } catch { /* private mode */ } },
};
const clean = (s) => s.toUpperCase().replace(/[^A-Z0-9_.-]/g, '').slice(0, 10);
nameInput.value = clean(store.get('name') || '');
nameInput.addEventListener('input', () => { nameInput.value = clean(nameInput.value); });

const mapReady = fetch('/map.bin').then((r) => r.arrayBuffer()).then((b) => { map = parseMap(b); });

startForm.addEventListener('submit', async (e) => {
  e.preventDefault();
  name = clean(nameInput.value);
  if (!name) { nameInput.focus(); return; }
  store.set('name', name);
  startForm.hidden = true;
  nameInput.blur();
  await mapReady;
  connect();
});

function connect() {
  state = 'connecting';
  ws = new WebSocket(`${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/ws`);
  ws.binaryType = 'arraybuffer';
  ws.onopen = join;
  ws.onmessage = (e) => onMessage(decode(e.data));
  ws.onclose = () => { state = 'closed'; ws = null; };
}

function join() {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(encodeJoin(name));
    state = 'joining';
  }
}

function sendInput() {
  seq = (seq % 65535) + 1;
  predictor.setSeq(seq);
  ws.send(encodeInput(seq, held, fire));
}

function onMessage(m) {
  switch (m.type) {
    case 'welcome':
      state = 'alive';
      awaitingFirst = true;
      seq = 0;
      dead = null;
      aliveSince = performance.now();
      if (held || fire) sendInput();
      break;
    case 'snapshot': {
      snap = m;
      const own = { x: m.ownX, y: m.ownY, dir: m.ownDir, cd: m.ownCd };
      if (awaitingFirst) {
        predictor.reset(own);
        if (seq) predictor.setSeq(seq);
        awaitingFirst = false;
      } else {
        predictor.reconcile(own, m.ackSeq, m.ackTicks, m.ownHasBullet, blocked);
      }
      break;
    }
    case 'names': for (const [id, n] of m.names) names.set(id, n); break;
    case 'board': board = m; break;
    case 'dead': state = 'dead'; dead = m; break;
    case 'full': state = 'full'; break;
  }
}

function blocked(x, y) {
  const b = snap.bounds;
  if (x < b.l || y < b.t || x + 3 > b.r || y + 3 > b.b) return true;
  for (let j = 0; j < 3; j++) for (let i = 0; i < 3; i++) if (map.isWall(x + i, y + j)) return true;
  return snap.tanks.some((t) => boxOverlap(x, y, t.x, t.y));
}

function bulletBlocked(x, y) {
  const b = snap.bounds;
  return x < b.l || y < b.t || x >= b.r || y >= b.b || map.isWall(x, y);
}

createInput((h, f) => {
  held = h;
  fire = f;
  if (state === 'alive') sendInput();
}, () => {
  if (state === 'dead' || state === 'full') join();
  else if (state === 'closed') connect();
});

setInterval(() => {
  if (state !== 'alive' || !snap || awaitingFirst) return;
  predictor.localTick(held ? held - 1 : null, fire, !snap.ownInvuln, blocked, bulletBlocked);
}, TICK_MS);

// Writes big pixel-font words into the LCD frame, centered, on a cleared band.
function bigText(words) {
  const top = 18 - words.length * 3;
  for (let y = top - 1; y < top + words.length * 6; y++) frame.fill(0, y * VIEW + 4, y * VIEW + 32);
  words.forEach((w, n) => {
    plotText(w, (VIEW - textWidth(w)) >> 1, top + n * 6, (x, y) => { frame[y * VIEW + x] = 1; });
  });
}

function overlay(now) {
  const blink = ((now / 400) | 0) % 2 === 0;
  if (state === 'dead' && dead) {
    bigText(['GAME', 'OVER']);
    const how = dead.cause === 1 ? 'SELF HIT' : dead.cause === 2 ? 'AFK' : dead.killer ? `BY ${dead.killer}` : '';
    const rec = dead.recordFlags ? 'NEW RECORD!' : '';
    return [how, `KILL ${dead.kills}  TIME ${clock(dead.secs)}`, rec, 'FIRE = PLAY'].filter(Boolean);
  }
  if (state === 'full') return ['SERVER FULL', 'FIRE = RETRY'];
  if (state === 'closed') return ['MAT KET NOI', 'FIRE = RETRY'];
  if (state === 'connecting' || state === 'joining') return ['...'];
  if (state === 'alive' && snap && snap.ownAfkWarn && blink) return ['AFK?'];
  return null;
}

function render(now) {
  requestAnimationFrame(render);
  frame.fill(0);
  const labels = [];
  if (snap && map) {
    const t = predictor.tank, b = snap.bounds;
    const vx = t.x - VIEW_HALF, vy = t.y - VIEW_HALF;
    for (let y = 0; y < VIEW; y++) for (let x = 0; x < VIEW; x++) {
      const wx = vx + x, wy = vy + y;
      frame[y * VIEW + x] = wx < b.l || wy < b.t || wx >= b.r || wy >= b.b || map.isWall(wx, wy) ? 1 : 0;
    }
    const blink = ((now / 125) | 0) % 2 === 0;
    const put = (x, y) => {
      x -= vx; y -= vy;
      if (x >= 0 && y >= 0 && x < VIEW && y < VIEW) frame[y * VIEW + x] = 1;
    };
    const drawTank = (x, y, dir, invuln) => {
      if (invuln && blink) return;
      const s = TANK_SPRITES[dir];
      for (let j = 0; j < 3; j++) for (let i = 0; i < 3; i++) if (s[j][i] === '1') put(x + i, y + j);
    };
    for (const o of snap.tanks) {
      drawTank(o.x, o.y, o.dir, o.invuln);
      labels.push({ cx: o.x - vx + 1.5, cy: o.y - vy, text: `${names.get(o.id) || '?'} ·${o.kills}` });
    }
    const alive = state === 'alive';
    if (alive) drawTank(t.x, t.y, t.dir, snap.ownInvuln);
    for (const bl of snap.bullets) if (!(alive && bl.own && predictor.bullet)) put(bl.x, bl.y);
    if (alive && predictor.bullet) put(predictor.bullet.x, predictor.bullet.y);
  } else if (state === 'start') {
    bigText(['TANK']);
  }
  const lines = overlay(now);
  lcd.draw(frame, labels, lines);
  const alive = state === 'alive';
  panel.update({
    kills: alive && snap ? snap.ownKills : dead ? dead.kills : 0,
    secs: alive ? Math.floor((now - aliveSince) / 1000) : dead ? dead.secs : 0,
    board,
  });
}

function layout() {
  const touch = matchMedia('(pointer: coarse)').matches;
  const portrait = innerHeight >= innerWidth;
  document.body.className = touch ? `touch ${portrait ? 'portrait' : 'landscape'}` : 'desk';
  const side = !touch ? Math.min(innerHeight * 0.9, innerWidth * 0.62)
    : portrait ? Math.min(innerWidth - 40, innerHeight * 0.52)
      : Math.min(innerHeight - 40, innerWidth * 0.45);
  const dpr = devicePixelRatio || 1;
  const cell = Math.max(3, Math.floor((side * dpr) / VIEW));
  const css = (cell * VIEW) / dpr;
  lcd.resize(cell, css);
  if (touch && portrait) panel.resize(css, Math.round(css * 0.3), dpr);
  else panel.resize(Math.round(css * (touch ? 0.34 : 0.45)), css, dpr);
}

addEventListener('resize', layout);
document.addEventListener('gesturestart', (e) => e.preventDefault());
layout();
requestAnimationFrame(render);
