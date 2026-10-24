//! Fixed wall map: one wall piece per 18×18 block, generated from a seed.

use crate::config::{BLOCK, BLOCK_MARGIN, MAP_SIZE};
use crate::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u8,
    pub h: u8,
}

pub struct WallMap {
    rects: Vec<Rect>,
    bits: Vec<u64>,
}

impl WallMap {
    pub fn generate(seed: u64) -> WallMap {
        let mut rng = Rng::new(seed);
        let mut rects = Vec::new();
        let inner = BLOCK - 2 * BLOCK_MARGIN;
        let blocks = MAP_SIZE / BLOCK;
        for by in 0..blocks {
            for bx in 0..blocks {
                let (ox, oy) = (bx * BLOCK + BLOCK_MARGIN, by * BLOCK + BLOCK_MARGIN);
                // Places a w×h box at a random spot inside the block's inner region.
                let place = |rng: &mut Rng, w: i32, h: i32| {
                    (ox + rng.range(0, inner - w + 1), oy + rng.range(0, inner - h + 1))
                };
                let roll = rng.range(0, 100);
                if roll < 90 {
                    let len = rng.range(4, 11);
                    let (w, h) = if roll < 45 { (len, 2) } else { (2, len) };
                    let (x, y) = place(&mut rng, w, h);
                    rects.push(rect(x, y, w, h));
                } else {
                    let (a, b) = (rng.range(4, 9), rng.range(4, 9));
                    let (x, y) = place(&mut rng, a, b);
                    let arm_y = if rng.chance(50) { y } else { y + b - 2 };
                    let arm_x = if rng.chance(50) { x } else { x + a - 2 };
                    rects.push(rect(x, arm_y, a, 2));
                    rects.push(rect(arm_x, y, 2, b));
                }
            }
        }
        let mut bits = vec![0u64; (MAP_SIZE as usize * MAP_SIZE as usize) / 64];
        for r in &rects {
            for y in r.y as i32..r.y as i32 + r.h as i32 {
                for x in r.x as i32..r.x as i32 + r.w as i32 {
                    let i = (y * MAP_SIZE + x) as usize;
                    bits[i / 64] |= 1 << (i % 64);
                }
            }
        }
        WallMap { rects, bits }
    }

    /// True for wall cells and for anything outside the map.
    pub fn is_wall(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= MAP_SIZE || y >= MAP_SIZE {
            return true;
        }
        let i = (y * MAP_SIZE + x) as usize;
        self.bits[i / 64] & (1 << (i % 64)) != 0
    }

    pub fn rects(&self) -> &[Rect] {
        &self.rects
    }

    /// The map.bin file: `BTMP`, version, size, count, then 6 bytes per rect.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(11 + 6 * self.rects.len());
        out.extend_from_slice(b"BTMP");
        out.push(1);
        out.extend_from_slice(&(MAP_SIZE as u16).to_le_bytes());
        out.extend_from_slice(&(self.rects.len() as u32).to_le_bytes());
        for r in &self.rects {
            out.extend_from_slice(&r.x.to_le_bytes());
            out.extend_from_slice(&r.y.to_le_bytes());
            out.push(r.w);
            out.push(r.h);
        }
        out
    }

    /// FNV-1a 32-bit hash of the encoded map.
    pub fn version(&self) -> u32 {
        fnv1a(&self.encode())
    }
}

pub fn fnv1a(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
    Rect { x: x as u16, y: y as u16, w: w as u8, h: h as u8 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{MAP_SEED, VIEW};
    use std::collections::VecDeque;

    #[test]
    fn deterministic() {
        assert_eq!(WallMap::generate(MAP_SEED).encode(), WallMap::generate(MAP_SEED).encode());
    }

    #[test]
    fn pieces_respect_margin() {
        let m = WallMap::generate(MAP_SEED);
        for r in m.rects() {
            let (x, y) = (r.x as i32, r.y as i32);
            let (bx, by) = (x / BLOCK, y / BLOCK);
            let (lo_x, lo_y) = (bx * BLOCK + BLOCK_MARGIN, by * BLOCK + BLOCK_MARGIN);
            let (hi_x, hi_y) = ((bx + 1) * BLOCK - BLOCK_MARGIN, (by + 1) * BLOCK - BLOCK_MARGIN);
            assert!(x >= lo_x && y >= lo_y, "{r:?}");
            assert!(x + r.w as i32 <= hi_x && y + r.h as i32 <= hi_y, "{r:?}");
        }
    }

    #[test]
    fn every_view_has_a_wall() {
        let m = WallMap::generate(MAP_SEED);
        let mut rng = Rng::new(1);
        for _ in 0..2000 {
            let (vx, vy) = (rng.range(0, MAP_SIZE - VIEW), rng.range(0, MAP_SIZE - VIEW));
            let found = (vy..vy + VIEW).any(|y| (vx..vx + VIEW).any(|x| m.is_wall(x, y)));
            assert!(found, "no wall in view at {vx},{vy}");
        }
    }

    #[test]
    fn free_space_connected() {
        let m = WallMap::generate(MAP_SEED);
        let (lo, hi) = (400, 600);
        let valid = |x: i32, y: i32| {
            x >= lo && y >= lo && x + 3 <= hi && y + 3 <= hi
                && (0..3).all(|j| (0..3).all(|i| !m.is_wall(x + i, y + j)))
        };
        let n = (hi - lo) as usize;
        let idx = |x: i32, y: i32| (y - lo) as usize * n + (x - lo) as usize;
        let mut seen = vec![false; n * n];
        let start = (lo..hi).flat_map(|y| (lo..hi).map(move |x| (x, y))).find(|&(x, y)| valid(x, y)).unwrap();
        let mut q = VecDeque::from([start]);
        seen[idx(start.0, start.1)] = true;
        while let Some((x, y)) = q.pop_front() {
            for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                let (nx, ny) = (x + dx, y + dy);
                if valid(nx, ny) && !seen[idx(nx, ny)] {
                    seen[idx(nx, ny)] = true;
                    q.push_back((nx, ny));
                }
            }
        }
        for y in lo..hi {
            for x in lo..hi {
                assert!(!valid(x, y) || seen[idx(x, y)], "unreachable tank spot {x},{y}");
            }
        }
    }

    #[test]
    fn encode_roundtrip_header() {
        let m = WallMap::generate(MAP_SEED);
        let b = m.encode();
        assert_eq!(&b[0..4], b"BTMP");
        assert_eq!(b[4], 1);
        assert_eq!(u16::from_le_bytes([b[5], b[6]]), 2048);
        let count = u32::from_le_bytes([b[7], b[8], b[9], b[10]]) as usize;
        assert_eq!(count, m.rects().len());
        assert_eq!(b.len(), 11 + 6 * count);
    }
}
