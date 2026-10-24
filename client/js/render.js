// LCD renderer: a 36×36 grid of "bricks", plus name labels and small overlay text.
import { VIEW } from './rules.js';
import { drawText, textWidth } from './font.js';

export const LCD_BG = '#9ea792', LCD_ON = '#1e231a';
const LCD_OFF = 'rgba(0,0,0,0.09)';

function brick(cell, color) {
  const c = document.createElement('canvas');
  c.width = c.height = cell;
  const g = c.getContext('2d');
  const lw = Math.max(1, Math.round(cell / 9)), m = Math.max(1, Math.round(cell * 0.08));
  g.strokeStyle = g.fillStyle = color;
  g.lineWidth = lw;
  g.strokeRect(m + lw / 2, m + lw / 2, cell - 2 * m - lw, cell - 2 * m - lw);
  const k = Math.round(cell * 0.3);
  g.fillRect(k, k, cell - 2 * k, cell - 2 * k);
  return c;
}

export function createLcd(canvas) {
  const ctx = canvas.getContext('2d');
  let cell = 10, on, off;
  return {
    // cellPx: device pixels per cell; cssSide: on-screen size of the whole LCD in CSS pixels.
    resize(cellPx, cssSide) {
      cell = cellPx;
      canvas.width = canvas.height = cell * VIEW;
      canvas.style.width = canvas.style.height = `${cssSide}px`;
      on = brick(cell, LCD_ON);
      off = brick(cell, LCD_OFF);
    },
    // frame: Uint8Array(36*36) of 0/1; labels: [{cx, cy, text}] in cells; lines: small centered text.
    draw(frame, labels, lines) {
      ctx.fillStyle = LCD_BG;
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      for (let i = 0; i < VIEW * VIEW; i++) {
        ctx.drawImage(frame[i] ? on : off, (i % VIEW) * cell, ((i / VIEW) | 0) * cell);
      }
      const fs = Math.max(9, Math.round(cell * 0.95));
      ctx.font = `bold ${fs}px ui-monospace, monospace`;
      ctx.textAlign = 'center';
      for (const l of labels) {
        const x = l.cx * cell, y = l.cy * cell - 3, w = ctx.measureText(l.text).width + 6;
        ctx.fillStyle = 'rgba(158,167,146,0.85)';
        ctx.fillRect(x - w / 2, y - fs, w, fs + 2);
        ctx.fillStyle = LCD_ON;
        ctx.fillText(l.text, x, y);
      }
      if (lines && lines.length) {
        const px = Math.max(1, Math.round(cell / 4)), lh = 7 * px;
        const w = Math.max(...lines.map(textWidth)) * px + 4 * px, h = lines.length * lh + 3 * px;
        const x0 = (canvas.width - w) / 2, y0 = canvas.height * 0.62;
        ctx.fillStyle = LCD_BG;
        ctx.fillRect(x0, y0, w, h);
        lines.forEach((t, n) => drawText(ctx, t, (canvas.width - textWidth(t) * px) / 2, y0 + 2 * px + n * lh, px, LCD_ON));
      }
    },
  };
}
