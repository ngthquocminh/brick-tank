//! Spatial hash over the map: buckets of 16×16 cells holding ids indexed by a single point.

use crate::config::MAP_SIZE;

const BUCKET: i32 = 16;
const SIDE: i32 = MAP_SIZE / BUCKET;

pub struct Grid {
    cells: Vec<Vec<u32>>,
}

impl Grid {
    pub fn new() -> Grid {
        Grid { cells: (0..SIDE * SIDE).map(|_| Vec::new()).collect() }
    }

    pub fn clear(&mut self) {
        for c in &mut self.cells {
            c.clear();
        }
    }

    pub fn insert(&mut self, id: u32, x: i32, y: i32) {
        let (bx, by) = ((x / BUCKET).clamp(0, SIDE - 1), (y / BUCKET).clamp(0, SIDE - 1));
        self.cells[(by * SIDE + bx) as usize].push(id);
    }

    /// Calls `f` for every id whose bucket intersects `[x0, x1) × [y0, y1)`. Callers filter exactly.
    pub fn query(&self, x0: i32, y0: i32, x1: i32, y1: i32, mut f: impl FnMut(u32)) {
        let bx0 = (x0 / BUCKET).clamp(0, SIDE - 1);
        let by0 = (y0 / BUCKET).clamp(0, SIDE - 1);
        let bx1 = ((x1 - 1) / BUCKET).clamp(0, SIDE - 1);
        let by1 = ((y1 - 1) / BUCKET).clamp(0, SIDE - 1);
        for by in by0..=by1 {
            for bx in bx0..=bx1 {
                for &id in &self.cells[(by * SIDE + bx) as usize] {
                    f(id);
                }
            }
        }
    }
}
