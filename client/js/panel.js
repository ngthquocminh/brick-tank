// Score column: KILL / TIME / ONLINE, then a leaderboard that rotates every 5 s.
import { drawText } from './font.js';
import { LCD_BG, LCD_ON } from './render.js';

const PAGES = [
  ['TOP KILL', 'aliveKills', 'myRankKills'],
  ['TOP TIME', 'aliveTime', 'myRankTime'],
  ['RECORD KILL', 'recKills'],
  ['RECORD TIME', 'recTime'],
];

export const clock = (s) => {
  const h = (s / 3600) | 0, m = ((s / 60) | 0) % 60, ss = s % 60;
  const p = (n) => String(n).padStart(2, '0');
  return h ? `${h}:${p(m)}:${p(ss)}` : `${p(m)}:${p(ss)}`;
};

export function createPanel(canvas) {
  const ctx = canvas.getContext('2d');
  let page = 0, flipped = performance.now();
  canvas.addEventListener('pointerdown', () => { page = (page + 1) % PAGES.length; flipped = performance.now(); });
  return {
    resize(w, h, dpr) {
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      canvas.style.width = `${w}px`;
      canvas.style.height = `${h}px`;
    },
    update({ kills, secs, board }) {
      if (performance.now() - flipped > 5000) { page = (page + 1) % PAGES.length; flipped = performance.now(); }
      const { width: w, height: h } = canvas;
      const px = Math.max(1, Math.floor(Math.min(w / 82, h / 56)));
      const lh = 7 * px, pad = 2 * px;
      ctx.fillStyle = LCD_BG;
      ctx.fillRect(0, 0, w, h);
      const lines = [`KILL ${kills}`, `TIME ${clock(secs)}`, `ONLINE ${board ? board.online : '-'}`, ''];
      if (board) {
        const [title, key, rankKey] = PAGES[page];
        lines.push(title);
        const isTime = key.endsWith('Time');
        const rows = Math.min(20, Math.max(0, Math.floor((h - pad) / lh) - lines.length));
        const mine = rankKey ? board[rankKey] : 0;
        const shown = mine > rows ? rows - 1 : rows;
        board[key].slice(0, Math.max(0, shown)).forEach(([name, v], i) => {
          lines.push(`${i + 1} ${name.padEnd(10)} ${isTime ? clock(v) : v}`);
        });
        if (mine > shown && rows > 0) lines.push(`#${mine} YOU`);
      }
      lines.forEach((t, n) => drawText(ctx, t, pad, pad + n * lh, px, LCD_ON));
    },
  };
}
