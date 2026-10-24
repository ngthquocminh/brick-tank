// Keyboard (arrows/WASD + Space) and on-screen buttons (multi-touch). Reports (heldCode, fire).
const KEYS = {
  ArrowUp: 1, KeyW: 1, ArrowRight: 2, KeyD: 2, ArrowDown: 3, KeyS: 3, ArrowLeft: 4, KeyA: 4,
};

// onChange(heldCode, fire) on every change; onAction() when fire/Enter is pressed (rejoin, retry).
export function createInput(onChange, onAction) {
  const pressed = []; // held direction codes, most recent last
  let fire = false, last = '';
  const emit = () => {
    const held = pressed.length ? pressed[pressed.length - 1] : 0;
    const key = `${held}${fire}`;
    if (key !== last) { last = key; onChange(held, fire); }
  };
  const press = (code) => { if (!pressed.includes(code)) pressed.push(code); emit(); };
  const release = (code) => { const i = pressed.indexOf(code); if (i >= 0) pressed.splice(i, 1); emit(); };
  const setFire = (v) => { if (v && !fire) onAction(); fire = v; emit(); };

  addEventListener('keydown', (e) => {
    if (e.target instanceof HTMLInputElement) return;
    if (KEYS[e.code]) { e.preventDefault(); press(KEYS[e.code]); }
    else if (e.code === 'Space') { e.preventDefault(); if (!e.repeat) setFire(true); }
    else if (e.code === 'Enter') onAction();
  });
  addEventListener('keyup', (e) => {
    if (KEYS[e.code]) release(KEYS[e.code]);
    else if (e.code === 'Space') setFire(false);
  });
  addEventListener('blur', () => { pressed.length = 0; fire = false; emit(); });

  for (const btn of document.querySelectorAll('[data-dir],[data-fire]')) {
    const code = Number(btn.dataset.dir || 0);
    const down = (e) => {
      e.preventDefault();
      btn.setPointerCapture(e.pointerId);
      btn.classList.add('on');
      if (code) press(code); else setFire(true);
    };
    const up = () => {
      btn.classList.remove('on');
      if (code) release(code); else setFire(false);
    };
    btn.addEventListener('pointerdown', down);
    btn.addEventListener('pointerup', up);
    btn.addEventListener('pointercancel', up);
    btn.addEventListener('lostpointercapture', up);
  }
}
