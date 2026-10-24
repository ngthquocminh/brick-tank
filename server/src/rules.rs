//! Movement rules shared with the client (see shared/movement_cases.json).

use crate::config::MOVE_COOLDOWN;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up = 0,
    Right = 1,
    Down = 2,
    Left = 3,
}

impl Dir {
    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir::Up => (0, -1),
            Dir::Right => (1, 0),
            Dir::Down => (0, 1),
            Dir::Left => (-1, 0),
        }
    }

    pub fn from_u8(v: u8) -> Option<Dir> {
        match v {
            0 => Some(Dir::Up),
            1 => Some(Dir::Right),
            2 => Some(Dir::Down),
            3 => Some(Dir::Left),
            _ => None,
        }
    }
}

/// Wire held-input code: 0 = none, 1–4 = Dir + 1.
pub fn held_from_code(code: u8) -> Result<Option<Dir>, ()> {
    match code {
        0 => Ok(None),
        1..=4 => Ok(Dir::from_u8(code - 1)),
        _ => Err(()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TankMove {
    pub x: i32,
    pub y: i32,
    pub dir: Dir,
    pub cd: u8,
}

/// One tick of movement. `blocked(x, y)` says whether a tank with top-left (x, y) collides.
pub fn step_move(t: &mut TankMove, held: Option<Dir>, blocked: impl Fn(i32, i32) -> bool) {
    if t.cd > 0 {
        t.cd -= 1;
        return;
    }
    let Some(d) = held else { return };
    if d != t.dir {
        t.dir = d;
    } else {
        let (dx, dy) = d.delta();
        if !blocked(t.x + dx, t.y + dy) {
            t.x += dx;
            t.y += dy;
        }
    }
    t.cd = MOVE_COOLDOWN;
}

/// The cell right in front of the barrel, where a new bullet appears.
pub fn muzzle(x: i32, y: i32, dir: Dir) -> (i32, i32) {
    match dir {
        Dir::Up => (x + 1, y - 1),
        Dir::Right => (x + 3, y + 1),
        Dir::Down => (x + 1, y + 3),
        Dir::Left => (x - 1, y + 1),
    }
}

/// Whether two 3×3 boxes given by their top-left corners overlap.
pub fn box_overlap(ax: i32, ay: i32, bx: i32, by: i32) -> bool {
    (ax - bx).abs() < 3 && (ay - by).abs() < 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn muzzle_positions() {
        assert_eq!(muzzle(10, 10, Dir::Up), (11, 9));
        assert_eq!(muzzle(10, 10, Dir::Right), (13, 11));
        assert_eq!(muzzle(10, 10, Dir::Down), (11, 13));
        assert_eq!(muzzle(10, 10, Dir::Left), (9, 11));
    }

    #[test]
    fn box_overlap_edges() {
        assert!(box_overlap(0, 0, 2, 2));
        assert!(!box_overlap(0, 0, 3, 0));
    }
}
