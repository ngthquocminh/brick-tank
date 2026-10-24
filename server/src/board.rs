//! Leaderboards of living tanks and the persisted all-time records.

use std::collections::HashMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::{BOARD_ALIVE_N, RECORDS_N, TICK_MS};
use crate::world::World;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    pub value: u32,
    pub date: String,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Records {
    pub kills: Vec<Entry>,
    pub time: Vec<Entry>,
}

pub struct AliveBoards {
    pub by_kills: Vec<(u32, u16)>,
    pub by_time: Vec<(u32, u32)>,
    pub rank_kills: HashMap<u32, u16>,
    pub rank_time: HashMap<u32, u16>,
}

impl Records {
    /// Missing or unreadable files give empty records (logged), never an error.
    pub fn load(path: &Path) -> Records {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                tracing::warn!("ignoring unreadable records file {}: {e}", path.display());
                Records::default()
            }),
            Err(_) => Records::default(),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, path)
    }

    /// Offers a finished life to both tables; returns whether it entered (kills, time).
    pub fn offer(&mut self, name: &str, kills: u32, secs: u32, date: &str) -> (bool, bool) {
        (insert(&mut self.kills, name, kills, date), insert(&mut self.time, name, secs, date))
    }
}

fn insert(table: &mut Vec<Entry>, name: &str, value: u32, date: &str) -> bool {
    if value == 0 {
        return false;
    }
    let pos = table.iter().position(|e| e.value < value).unwrap_or(table.len());
    if pos >= RECORDS_N {
        return false;
    }
    table.insert(pos, Entry { name: name.to_string(), value, date: date.to_string() });
    table.truncate(RECORDS_N);
    true
}

pub fn alive_secs(world: &World, born_tick: u64) -> u32 {
    ((world.tick - born_tick) * TICK_MS / 1000) as u32
}

pub fn alive_boards(world: &World) -> AliveBoards {
    let mut by_kills: Vec<(u32, u16)> = world.tanks().map(|t| (t.id, t.kills)).collect();
    by_kills.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut by_born: Vec<(u32, u64)> = world.tanks().map(|t| (t.id, t.born_tick)).collect();
    by_born.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

    let rank = |ids: &mut dyn Iterator<Item = u32>| -> HashMap<u32, u16> {
        ids.enumerate().map(|(i, id)| (id, (i + 1).min(u16::MAX as usize) as u16)).collect()
    };
    let rank_kills = rank(&mut by_kills.iter().map(|e| e.0));
    let rank_time = rank(&mut by_born.iter().map(|e| e.0));
    by_kills.truncate(BOARD_ALIVE_N);
    let by_time = by_born.iter().take(BOARD_ALIVE_N).map(|&(id, born)| (id, alive_secs(world, born))).collect();
    AliveBoards { by_kills, by_time, rank_kills, rank_time }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Dir;
    use crate::world::tests::world;

    fn values(v: &[Entry]) -> Vec<u32> {
        v.iter().map(|e| e.value).collect()
    }

    #[test]
    fn offer_orders_and_caps() {
        let mut r = Records::default();
        for k in [3, 9, 1, 7, 5, 8, 2] {
            r.offer("A", k, 10, "2026-10-24");
        }
        assert_eq!(values(&r.kills), vec![9, 8, 7, 5, 3]);
    }

    #[test]
    fn offer_zero_rejected() {
        let mut r = Records::default();
        assert_eq!(r.offer("A", 0, 0, "d"), (false, false));
        assert!(r.kills.is_empty() && r.time.is_empty());
    }

    #[test]
    fn offer_tie_does_not_displace_when_full() {
        let mut r = Records::default();
        for (i, k) in [5, 5, 5, 5, 5].iter().enumerate() {
            r.offer(&format!("P{i}"), *k, 0, "d");
        }
        assert_eq!(r.offer("NEW", 5, 0, "d"), (false, false));
        assert_eq!(r.kills[0].name, "P0");
        assert_eq!(r.kills.len(), 5);
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("records.json");
        let mut r = Records::default();
        r.offer("LAN", 12, 340, "2026-10-24");
        r.save(&path).unwrap();
        let back = Records::load(&path);
        assert_eq!(back.kills, r.kills);
        assert_eq!(back.time, r.time);
    }

    #[test]
    fn load_corrupt_file_gives_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("records.json");
        std::fs::write(&path, "{not json").unwrap();
        let r = Records::load(&path);
        assert!(r.kills.is_empty() && r.time.is_empty());
    }

    #[test]
    fn alive_boards_ranks() {
        let mut w = world();
        let a = w.place("A".into(), 1000, 1000, Dir::Up);
        let b = w.place("B".into(), 1010, 1000, Dir::Up);
        let c = w.place("C".into(), 1020, 1000, Dir::Up);
        w.set_kills(a, 5);
        w.set_kills(b, 2);
        w.set_kills(c, 5);
        let boards = alive_boards(&w);
        assert_eq!(boards.by_kills, vec![(a, 5), (c, 5), (b, 2)]);
        assert_eq!((boards.rank_kills[&a], boards.rank_kills[&b], boards.rank_kills[&c]), (1, 3, 2));
        assert_eq!(boards.rank_time[&a], 1);
    }
}
