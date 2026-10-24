// Movement rules shared with the server (see shared/movement_cases.json).
export const VIEW = 36, VIEW_HALF = 17, DX_OFFSET = 8, TICK_MS = 50, MOVE_COOLDOWN = 1, BULLET_RANGE = 25;
export const DIR = { UP: 0, RIGHT: 1, DOWN: 2, LEFT: 3 };
export const DELTA = [[0, -1], [1, 0], [0, 1], [-1, 0]];
export const TANK_SPRITES = [
  ['010', '111', '101'],
  ['110', '011', '110'],
  ['101', '111', '010'],
  ['011', '110', '011'],
];

// One tick of movement; `blocked(x, y)` says whether a tank with top-left (x, y) collides.
export function stepMove(t, held, blocked) {
  if (t.cd > 0) { t.cd--; return; }
  if (held === null || held === undefined) return;
  if (held !== t.dir) {
    t.dir = held;
  } else {
    const [dx, dy] = DELTA[held];
    if (!blocked(t.x + dx, t.y + dy)) { t.x += dx; t.y += dy; }
  }
  t.cd = MOVE_COOLDOWN;
}

export function muzzle(x, y, dir) {
  return [[x + 1, y - 1], [x + 3, y + 1], [x + 1, y + 3], [x - 1, y + 1]][dir];
}

export function boxOverlap(ax, ay, bx, by) {
  return Math.abs(ax - bx) < 3 && Math.abs(ay - by) < 3;
}
