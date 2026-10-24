//! The open play area: a rectangle with four independent sides.

use crate::config::{AREA_PER_PLAYER, MAP_SIZE, SHRINK_EVERY, SHRINK_SLACK, S_MIN};

/// Half-open: cells with `l <= x < r` and `t <= y < b` are open.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Bounds {
    pub l: i32,
    pub t: i32,
    pub r: i32,
    pub b: i32,
}

impl Bounds {
    pub fn contains_cell(&self, x: i32, y: i32) -> bool {
        x >= self.l && x < self.r && y >= self.t && y < self.b
    }

    /// Whether the whole 3×3 box with top-left (x, y) is open.
    pub fn contains_box(&self, x: i32, y: i32) -> bool {
        x >= self.l && x + 3 <= self.r && y >= self.t && y + 3 <= self.b
    }

    pub fn area(&self) -> i64 {
        (self.r - self.l).max(0) as i64 * (self.b - self.t).max(0) as i64
    }

    pub fn intersect(&self, o: &Bounds) -> Bounds {
        Bounds { l: self.l.max(o.l), t: self.t.max(o.t), r: self.r.min(o.r), b: self.b.min(o.b) }
    }
}

/// The square the open area should converge to for `n` players.
pub fn target(n: usize) -> Bounds {
    let want = (S_MIN as i64 * S_MIN as i64).max(n as i64 * AREA_PER_PLAYER);
    let mut s = (want as f64).sqrt() as i64;
    while s * s < want {
        s += 1;
    }
    while s > 0 && (s - 1) * (s - 1) >= want {
        s -= 1;
    }
    s += s % 2;
    let s = (s as i32).clamp(S_MIN, MAP_SIZE);
    let lo = MAP_SIZE / 2 - s / 2;
    Bounds { l: lo, t: lo, r: lo + s, b: lo + s }
}

/// Moves each side one step toward `target`. Growing happens every tick; shrinking only every
/// `SHRINK_EVERY` ticks, past the slack, and never past a tank (`tanks` are top-left corners).
pub fn update(b: &mut Bounds, target: &Bounds, tick: u64, tanks: &[(i32, i32)]) {
    if b.l > target.l {
        b.l -= 1;
    }
    if b.t > target.t {
        b.t -= 1;
    }
    if b.r < target.r {
        b.r += 1;
    }
    if b.b < target.b {
        b.b += 1;
    }
    if tick % SHRINK_EVERY != 0 || (b.area() as f64) <= SHRINK_SLACK * target.area() as f64 {
        return;
    }
    if b.l < target.l && tanks.iter().all(|&(x, _)| x >= b.l + 1) {
        b.l += 1;
    }
    if b.t < target.t && tanks.iter().all(|&(_, y)| y >= b.t + 1) {
        b.t += 1;
    }
    if b.r > target.r && tanks.iter().all(|&(x, _)| x + 3 <= b.r - 1) {
        b.r -= 1;
    }
    if b.b > target.b && tanks.iter().all(|&(_, y)| y + 3 <= b.b - 1) {
        b.b -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn side(b: &Bounds) -> i32 {
        b.r - b.l
    }

    #[test]
    fn target_min_and_growth() {
        let t0 = target(0);
        assert_eq!((t0.l, t0.t, t0.r, t0.b), (994, 994, 1054, 1054));
        assert_eq!(side(&target(100)), 200);
        assert_eq!(side(&target(101)), 202);
        assert_eq!(target(1_000_000), Bounds { l: 0, t: 0, r: 2048, b: 2048 });
    }

    #[test]
    fn grows_one_per_tick() {
        let mut b = target(0);
        let goal = target(100);
        update(&mut b, &goal, 1, &[]);
        assert_eq!(b, Bounds { l: 993, t: 993, r: 1055, b: 1055 });
        for tick in 2..=70 {
            update(&mut b, &goal, tick, &[]);
        }
        assert_eq!(b, goal);
    }

    #[test]
    fn shrink_waits_for_slack_and_cadence() {
        let mut b = target(100);
        let near = target(90);
        for tick in 1..=200 {
            update(&mut b, &near, tick, &[]);
        }
        assert_eq!(b, target(100));

        let small = target(10);
        for tick in 1..40 {
            update(&mut b, &small, tick, &[]);
        }
        assert_eq!(b, target(100));
        update(&mut b, &small, 40, &[]);
        assert_eq!(side(&b), 198);
    }

    #[test]
    fn tank_holds_its_side_only() {
        let start = target(100);
        let mut b = start;
        let tank = (start.l, 1000);
        update(&mut b, &target(0), 40, &[tank]);
        assert_eq!(b.l, start.l);
        assert_eq!((b.t, b.r, b.b), (start.t + 1, start.r - 1, start.b - 1));
    }
}
