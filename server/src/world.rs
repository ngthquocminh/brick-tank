//! The authoritative game world: tanks, bullets, the open area, and one tick of simulation.

use std::collections::HashMap;
use std::sync::Arc;

use crate::area::{self, Bounds};
use crate::config::*;
use crate::grid::Grid;
use crate::map::WallMap;
use crate::rng::Rng;
use crate::rules::{box_overlap, muzzle, step_move, Dir, TankMove};

pub struct Tank {
    pub id: u32,
    pub name: Arc<str>,
    pub mv: TankMove,
    pub held: Option<Dir>,
    pub fire: bool,
    pub kills: u16,
    pub born_tick: u64,
    pub invuln_until: u64,
    pub last_input_tick: u64,
    pub has_bullet: bool,
}

pub struct Bullet {
    pub owner: u32,
    pub x: i32,
    pub y: i32,
    pub px: i32,
    pub py: i32,
    pub dir: Dir,
    pub range_left: u8,
    pub fresh: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cause {
    Shot { killer: u32 },
    SelfHit,
    Afk,
    Disconnect,
}

#[derive(Debug)]
pub struct Death {
    pub id: u32,
    pub name: Arc<str>,
    pub kills: u16,
    pub alive_ticks: u64,
    pub cause: Cause,
    pub killer_name: Option<Arc<str>>,
}

pub struct World {
    pub tick: u64,
    pub bounds: Bounds,
    pub map: Arc<WallMap>,
    tanks: Vec<Tank>,
    index: HashMap<u32, usize>,
    bullets: Vec<Bullet>,
    grid: Grid,
    bullet_grid: Grid,
    rng: Rng,
    next_id: u32,
}

impl World {
    pub fn new(map: Arc<WallMap>, seed: u64) -> World {
        World {
            tick: 0,
            bounds: area::target(0),
            map,
            tanks: Vec::new(),
            index: HashMap::new(),
            bullets: Vec::new(),
            grid: Grid::new(),
            bullet_grid: Grid::new(),
            rng: Rng::new(seed),
            next_id: 1,
        }
    }

    pub fn tank(&self, id: u32) -> Option<&Tank> {
        self.index.get(&id).map(|&i| &self.tanks[i])
    }

    pub fn tanks(&self) -> impl Iterator<Item = &Tank> {
        self.tanks.iter()
    }

    pub fn tank_count(&self) -> usize {
        self.tanks.len()
    }

    pub fn bullets(&self) -> &[Bullet] {
        &self.bullets
    }

    pub fn is_invulnerable(&self, t: &Tank) -> bool {
        self.tick < t.invuln_until
    }

    pub fn afk_warning(&self, t: &Tank) -> bool {
        self.tick - t.last_input_tick >= AFK_WARN_TICKS
    }

    pub fn set_input(&mut self, id: u32, held: Option<Dir>, fire: bool) {
        if let Some(&i) = self.index.get(&id) {
            let t = &mut self.tanks[i];
            t.held = held;
            t.fire = fire;
            t.last_input_tick = self.tick;
        }
    }

    /// Spawns a tank at the safest of a few random spots; `None` only when no free spot exists.
    pub fn spawn(&mut self, name: Arc<str>) -> Option<u32> {
        let mut region = self.bounds.intersect(&area::target(self.tanks.len() + 1));
        if region.r - region.l < 3 || region.b - region.t < 3 {
            region = self.bounds;
        }
        let (x_hi, y_hi) = (region.r - 2, region.b - 2);
        if x_hi <= region.l || y_hi <= region.t {
            return None;
        }
        let mut best: Option<(i32, i32, i32)> = None;
        for _ in 0..SPAWN_CANDIDATES {
            let (x, y) = (self.rng.range(region.l, x_hi), self.rng.range(region.t, y_hi));
            if !self.tank_fits(x, y, None) {
                continue;
            }
            let score = self.nearest_threat(x, y);
            if best.map_or(true, |(_, _, s)| score > s) {
                best = Some((x, y, score));
            }
        }
        let (x, y) = match best {
            Some((x, y, _)) => (x, y),
            None => self.scan_free(&region)?,
        };
        let dir = Dir::from_u8(self.rng.range(0, 4) as u8).unwrap();
        let id = self.insert_tank(name, x, y, dir);
        self.tanks[self.index[&id]].invuln_until = self.tick + INVULN_TICKS;
        Some(id)
    }

    pub fn remove(&mut self, id: u32, cause: Cause) -> Option<Death> {
        let i = self.index.remove(&id)?;
        let t = self.tanks.swap_remove(i);
        if i < self.tanks.len() {
            self.index.insert(self.tanks[i].id, i);
        }
        let killer_name = match cause {
            Cause::Shot { killer } => self.tank(killer).map(|k| k.name.clone()),
            _ => None,
        };
        Some(Death { id, name: t.name, kills: t.kills, alive_ticks: self.tick - t.born_tick, cause, killer_name })
    }

    pub fn step(&mut self) -> Vec<Death> {
        self.tick += 1;
        let mut deaths = Vec::new();

        let afk: Vec<u32> = self.tanks.iter()
            .filter(|t| self.tick - t.last_input_tick >= AFK_KILL_TICKS)
            .map(|t| t.id)
            .collect();
        for id in afk {
            deaths.extend(self.remove(id, Cause::Afk));
        }
        self.rebuild_grid();

        self.move_tanks();
        self.rebuild_grid();

        let corners: Vec<(i32, i32)> = self.tanks.iter().map(|t| (t.mv.x, t.mv.y)).collect();
        area::update(&mut self.bounds, &area::target(self.tanks.len()), self.tick, &corners);
        self.rebuild_grid();
        deaths
    }

    /// Ids of tanks whose box intersects, and indices of bullets inside, `[x0, x1) × [y0, y1)`.
    pub fn query_view(&self, x0: i32, y0: i32, x1: i32, y1: i32, tanks: &mut Vec<u32>, bullets: &mut Vec<usize>) {
        self.grid.query(x0 - 2, y0 - 2, x1, y1, |id| {
            let t = &self.tanks[self.index[&id]];
            if t.mv.x + 3 > x0 && t.mv.x < x1 && t.mv.y + 3 > y0 && t.mv.y < y1 {
                tanks.push(id);
            }
        });
        self.bullet_grid.query(x0, y0, x1, y1, |i| {
            let b = &self.bullets[i as usize];
            if b.x >= x0 && b.x < x1 && b.y >= y0 && b.y < y1 {
                bullets.push(i as usize);
            }
        });
    }

    fn insert_tank(&mut self, name: Arc<str>, x: i32, y: i32, dir: Dir) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.index.insert(id, self.tanks.len());
        self.tanks.push(Tank {
            id,
            name,
            mv: TankMove { x, y, dir, cd: 0 },
            held: None,
            fire: false,
            kills: 0,
            born_tick: self.tick,
            invuln_until: 0,
            last_input_tick: self.tick,
            has_bullet: false,
        });
        self.grid.insert(id, x, y);
        id
    }

    fn box_blocked_static(&self, x: i32, y: i32) -> bool {
        !self.bounds.contains_box(x, y) || (0..3).any(|j| (0..3).any(|i| self.map.is_wall(x + i, y + j)))
    }

    fn tank_fits(&self, x: i32, y: i32, ignore: Option<u32>) -> bool {
        if self.box_blocked_static(x, y) {
            return false;
        }
        let mut free = true;
        self.grid.query(x - 2, y - 2, x + 3, y + 3, |id| {
            let t = &self.tanks[self.index[&id]];
            if Some(id) != ignore && box_overlap(x, y, t.mv.x, t.mv.y) {
                free = false;
            }
        });
        free
    }

    /// Chebyshev distance from (x, y) to the nearest tank or bullet, capped at twice the safe distance.
    fn nearest_threat(&self, x: i32, y: i32) -> i32 {
        let r = SPAWN_SAFE_DIST * 2;
        let mut best = r;
        self.grid.query(x - r, y - r, x + r, y + r, |id| {
            let t = &self.tanks[self.index[&id]];
            best = best.min((t.mv.x - x).abs().max((t.mv.y - y).abs()));
        });
        self.bullet_grid.query(x - r, y - r, x + r, y + r, |i| {
            let b = &self.bullets[i as usize];
            best = best.min((b.x - x - 1).abs().max((b.y - y - 1).abs()));
        });
        best
    }

    fn scan_free(&mut self, region: &Bounds) -> Option<(i32, i32)> {
        let (w, h) = (region.r - region.l - 2, region.b - region.t - 2);
        let total = w as i64 * h as i64;
        let start = self.rng.next_u64() as i64 % total;
        (0..total).map(|k| (start + k) % total).find_map(|k| {
            let (x, y) = (region.l + (k % w as i64) as i32, region.t + (k / w as i64) as i32);
            self.tank_fits(x, y, None).then_some((x, y))
        })
    }

    fn move_tanks(&mut self) {
        let desired: Vec<TankMove> = self.tanks.iter().map(|t| {
            let mut m = t.mv;
            step_move(&mut m, t.held, |x, y| self.box_blocked_static(x, y));
            m
        }).collect();
        for i in 0..self.tanks.len() {
            let (cur, mut want) = (self.tanks[i].mv, desired[i]);
            if (want.x, want.y) != (cur.x, cur.y) {
                let id = self.tanks[i].id;
                let mut clash = false;
                self.grid.query(want.x - 4, want.y - 4, want.x + 5, want.y + 5, |oid| {
                    if oid == id {
                        return;
                    }
                    let j = self.index[&oid];
                    let (o, od) = (self.tanks[j].mv, desired[j]);
                    clash |= box_overlap(want.x, want.y, o.x, o.y) || box_overlap(want.x, want.y, od.x, od.y);
                });
                if clash {
                    want.x = cur.x;
                    want.y = cur.y;
                }
            }
            self.tanks[i].mv = want;
        }
    }

    fn rebuild_grid(&mut self) {
        self.grid.clear();
        for t in &self.tanks {
            self.grid.insert(t.id, t.mv.x, t.mv.y);
        }
        self.bullet_grid.clear();
        for (i, b) in self.bullets.iter().enumerate() {
            self.bullet_grid.insert(i as u32, b.x, b.y);
        }
    }

    #[cfg(test)]
    pub(crate) fn place(&mut self, name: Arc<str>, x: i32, y: i32, dir: Dir) -> u32 {
        self.insert_tank(name, x, y, dir)
    }

    #[cfg(test)]
    pub(crate) fn set_bounds(&mut self, b: Bounds) {
        self.bounds = b;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn world() -> World {
        World::new(Arc::new(WallMap::generate(MAP_SEED)), 1)
    }

    /// Top-left of a wall-free `w`×`h` strip inside the starting area.
    pub(crate) fn free_strip(w: &World, sw: i32, sh: i32) -> (i32, i32) {
        let b = w.bounds;
        for y in b.t..b.b - sh {
            for x in b.l..b.r - sw {
                if (0..sh).all(|j| (0..sw).all(|i| !w.map.is_wall(x + i, y + j))) {
                    return (x, y);
                }
            }
        }
        panic!("no free strip");
    }

    fn box_clear(w: &World, x: i32, y: i32) -> bool {
        (0..3).all(|j| (0..3).all(|i| !w.map.is_wall(x + i, y + j)))
    }

    #[test]
    fn spawn_inside_area_not_on_walls() {
        let mut w = world();
        for _ in 0..200 {
            let id = w.spawn("A".into()).expect("spawn");
            let t = w.tank(id).unwrap();
            let (x, y) = (t.mv.x, t.mv.y);
            assert!(w.bounds.contains_box(x, y));
            assert!(box_clear(&w, x, y));
            assert!(w.tanks().all(|o| o.id == id || !box_overlap(x, y, o.mv.x, o.mv.y)));
            w.step();
        }
    }

    #[test]
    fn spawn_prefers_distance() {
        let mut w = world();
        w.place("C".into(), 1023, 1023, Dir::Up);
        let id = w.spawn("A".into()).unwrap();
        let t = w.tank(id).unwrap();
        let d = (t.mv.x - 1023).abs().max((t.mv.y - 1023).abs());
        assert!(d >= SPAWN_SAFE_DIST, "distance {d}");
    }

    #[test]
    fn spawn_falls_back_when_crowded() {
        let mut w = world();
        let (x, y) = free_strip(&w, 9, 3);
        w.set_bounds(Bounds { l: x, t: y, r: x + 9, b: y + 3 });
        w.place("L".into(), x, y, Dir::Up);
        w.place("M".into(), x + 3, y, Dir::Up);
        let id = w.spawn("A".into()).expect("one slot left");
        let t = w.tank(id).unwrap();
        assert_eq!((t.mv.x, t.mv.y), (x + 6, y));
        assert!(w.spawn("B".into()).is_none());
    }

    #[test]
    fn two_tanks_same_target_both_blocked() {
        let mut w = world();
        let (x, y) = free_strip(&w, 7, 3);
        let a = w.place("A".into(), x, y, Dir::Right);
        let b = w.place("B".into(), x + 4, y, Dir::Left);
        w.set_input(a, Some(Dir::Right), false);
        w.set_input(b, Some(Dir::Left), false);
        w.step();
        assert_eq!(w.tank(a).unwrap().mv.x, x);
        assert_eq!(w.tank(b).unwrap().mv.x, x + 4);
    }

    #[test]
    fn afk_removed_after_1200_ticks() {
        let mut w = world();
        let id = w.place("A".into(), 1000, 1000, Dir::Up);
        for _ in 0..1199 {
            assert!(w.step().is_empty());
        }
        let t = w.tank(id).unwrap();
        assert!(w.afk_warning(t));
        let deaths = w.step();
        assert_eq!(deaths.len(), 1);
        assert_eq!(deaths[0].cause, Cause::Afk);
        assert!(w.tank(id).is_none());
    }

    #[test]
    fn afk_warning_starts_at_1000() {
        let mut w = world();
        let id = w.place("A".into(), 1000, 1000, Dir::Up);
        for _ in 0..999 {
            w.step();
        }
        assert!(!w.afk_warning(w.tank(id).unwrap()));
        w.step();
        assert!(w.afk_warning(w.tank(id).unwrap()));
    }

    #[test]
    fn invulnerable_for_60_ticks() {
        let mut w = world();
        let id = w.spawn("A".into()).unwrap();
        for _ in 0..59 {
            w.step();
            assert!(w.is_invulnerable(w.tank(id).unwrap()));
        }
        w.step();
        assert!(!w.is_invulnerable(w.tank(id).unwrap()));
    }
}
