//! The game thread: owns the world, applies client messages, ticks, and fans out snapshots.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use rayon::prelude::*;
use tokio::sync::{mpsc, watch};

use crate::board::{alive_boards, Records};
use crate::config::*;
use crate::map::WallMap;
use crate::protocol::*;
use crate::world::{Cause, Death, World};

pub struct Outbox {
    pub snapshot: watch::Sender<Bytes>,
    pub reliable: mpsc::Sender<Bytes>,
}

pub enum Inbound {
    Connect { conn: u64, out: Outbox },
    Msg { conn: u64, msg: ClientMsg },
    Disconnect { conn: u64 },
    Shutdown,
}

pub struct GameConfig {
    pub records_path: PathBuf,
    pub max_players: usize,
    pub seed: u64,
}

struct Client {
    out: Outbox,
    tank: Option<u32>,
    known: HashSet<u32>,
    last_seq: u16,
    seq_start_tick: u64,
    msgs_in_window: u32,
    drop_me: bool,
}

pub struct Game {
    world: World,
    map_version: u32,
    records: Records,
    cfg: GameConfig,
    clients: HashMap<u64, Client>,
}

impl Game {
    pub fn new(cfg: GameConfig, map: Arc<WallMap>) -> Game {
        Game {
            map_version: map.version(),
            world: World::new(map, cfg.seed),
            records: Records::load(&cfg.records_path),
            cfg,
            clients: HashMap::new(),
        }
    }

    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    pub fn handle(&mut self, ev: Inbound) {
        match ev {
            Inbound::Connect { conn, out } => {
                let client = Client {
                    out,
                    tank: None,
                    known: HashSet::new(),
                    last_seq: 0,
                    seq_start_tick: 0,
                    msgs_in_window: 0,
                    drop_me: false,
                };
                self.clients.insert(conn, client);
            }
            Inbound::Msg { conn, msg } => self.on_msg(conn, msg),
            Inbound::Disconnect { conn } => self.drop_client(conn),
            Inbound::Shutdown => self.shutdown(),
        }
    }

    fn on_msg(&mut self, conn: u64, msg: ClientMsg) {
        let tick = self.world.tick;
        let Some(c) = self.clients.get_mut(&conn) else { return };
        match msg {
            ClientMsg::Join { name } => {
                if c.tank.is_some() {
                    return;
                }
                let spawned = if self.world.tank_count() >= self.cfg.max_players {
                    None
                } else {
                    self.world.spawn(name.into())
                };
                let reply = match spawned {
                    Some(id) => {
                        c.tank = Some(id);
                        c.last_seq = 0;
                        c.seq_start_tick = tick + 1;
                        c.known.clear();
                        encode_welcome(id, self.map_version)
                    }
                    None => encode_full(),
                };
                if c.out.reliable.try_send(reply).is_err() {
                    c.drop_me = true;
                }
            }
            ClientMsg::Input { seq, held, fire } => {
                c.msgs_in_window += 1;
                if c.msgs_in_window > INPUT_RATE_MAX {
                    self.drop_client(conn);
                    return;
                }
                let Some(id) = c.tank else { return };
                if seq != c.last_seq {
                    c.last_seq = seq;
                    c.seq_start_tick = tick + 1;
                }
                self.world.set_input(id, held, fire);
            }
        }
    }

    /// Removes the client and its tank (counted as a death for records).
    fn drop_client(&mut self, conn: u64) {
        if let Some(c) = self.clients.remove(&conn) {
            if let Some(d) = c.tank.and_then(|id| self.world.remove(id, Cause::Disconnect)) {
                self.record(&d);
            }
        }
    }

    /// Offers a finished life to the records, saving on change; returns DEAD's record flags.
    fn record(&mut self, d: &Death) -> u8 {
        let secs = (d.alive_ticks * TICK_MS / 1000) as u32;
        let (rk, rt) = self.records.offer(&d.name, d.kills as u32, secs, &utc_date());
        if rk || rt {
            if let Err(e) = self.records.save(&self.cfg.records_path) {
                tracing::error!("saving records: {e}");
            }
        }
        rk as u8 | (rt as u8) << 1
    }

    pub fn tick(&mut self) {
        for d in self.world.step() {
            let flags = self.record(&d);
            let cause = match d.cause {
                Cause::Shot { .. } => 0,
                Cause::SelfHit => 1,
                Cause::Afk => 2,
                Cause::Disconnect => continue,
            };
            let secs = (d.alive_ticks * TICK_MS / 1000) as u32;
            let msg = encode_dead(cause, d.killer_name.as_deref().unwrap_or(""), d.kills, secs, flags);
            if let Some(c) = self.clients.values_mut().find(|c| c.tank == Some(d.id)) {
                c.tank = None;
                if c.out.reliable.try_send(msg).is_err() {
                    c.drop_me = true;
                }
            }
        }

        let world = &self.world;
        self.clients.par_iter_mut().for_each(|(_, c)| send_snapshot(world, c));

        if self.world.tick % BOARD_EVERY == 0 {
            self.send_boards();
        }
        if self.world.tick % 20 == 0 {
            for c in self.clients.values_mut() {
                c.msgs_in_window = 0;
            }
        }
        let dropped: Vec<u64> = self.clients.iter().filter(|(_, c)| c.drop_me).map(|(&k, _)| k).collect();
        for conn in dropped {
            self.drop_client(conn);
        }
    }

    fn send_boards(&mut self) {
        let world = &self.world;
        let boards = alive_boards(world);
        let name = |id: u32| world.tank(id).map(|t| &*t.name).unwrap_or("");
        let alive_kills: Vec<(&str, u16)> = boards.by_kills.iter().map(|&(id, k)| (name(id), k)).collect();
        let alive_time: Vec<(&str, u32)> = boards.by_time.iter().map(|&(id, s)| (name(id), s)).collect();
        let rec_kills: Vec<(&str, u16)> = self.records.kills.iter().map(|e| (&*e.name, e.value.min(u16::MAX as u32) as u16)).collect();
        let rec_time: Vec<(&str, u32)> = self.records.time.iter().map(|e| (&*e.name, e.value)).collect();
        let online = world.tank_count().min(u16::MAX as usize) as u16;
        let common = encode_board_common(online, &alive_kills, &alive_time, &rec_kills, &rec_time);
        for c in self.clients.values_mut() {
            let rank = |m: &HashMap<u32, u16>| c.tank.and_then(|id| m.get(&id).copied()).unwrap_or(0);
            let mut out = Vec::with_capacity(common.len() + 4);
            encode_board(&mut out, &common, rank(&boards.rank_kills), rank(&boards.rank_time));
            if c.out.reliable.try_send(out.into()).is_err() {
                c.drop_me = true;
            }
        }
    }

    /// Offers every living tank to the records and saves them.
    pub fn shutdown(&mut self) {
        let ids: Vec<u32> = self.world.tanks().map(|t| t.id).collect();
        for id in ids {
            if let Some(d) = self.world.remove(id, Cause::Disconnect) {
                self.record(&d);
            }
        }
        if let Err(e) = self.records.save(&self.cfg.records_path) {
            tracing::error!("saving records: {e}");
        }
    }
}

fn send_snapshot(world: &World, c: &mut Client) {
    let Some(t) = c.tank.and_then(|id| world.tank(id)) else { return };
    let (vx, vy) = (t.mv.x - VIEW_HALF, t.mv.y - VIEW_HALF);
    let (mut ids, mut bullet_idx) = (Vec::new(), Vec::new());
    world.query_view(vx - VIEW_MARGIN, vy - VIEW_MARGIN, vx + VIEW + VIEW_MARGIN, vy + VIEW + VIEW_MARGIN, &mut ids, &mut bullet_idx);
    let mut tanks = Vec::with_capacity(ids.len());
    let mut new_names: Vec<(u32, &str)> = Vec::new();
    for id in ids {
        if id == t.id {
            continue;
        }
        let o = world.tank(id).unwrap();
        tanks.push(TankView {
            id,
            dx: (o.mv.x - vx + DX_OFFSET) as u8,
            dy: (o.mv.y - vy + DX_OFFSET) as u8,
            dir: o.mv.dir,
            invuln: world.is_invulnerable(o),
            kills: o.kills,
        });
        if c.known.insert(id) {
            new_names.push((id, &o.name));
        }
    }
    let bullets: Vec<BulletView> = bullet_idx.iter().map(|&i| {
        let b = &world.bullets()[i];
        BulletView { dx: (b.x - vx + DX_OFFSET) as u8, dy: (b.y - vy + DX_OFFSET) as u8, own: b.owner == t.id }
    }).collect();
    if !new_names.is_empty() && c.out.reliable.try_send(encode_names(&new_names)).is_err() {
        c.drop_me = true;
    }
    let own = OwnView {
        x: t.mv.x as u16,
        y: t.mv.y as u16,
        dir: t.mv.dir,
        invuln: world.is_invulnerable(t),
        afk_warn: world.afk_warning(t),
        has_bullet: t.has_bullet,
        cd: t.mv.cd,
        kills: t.kills,
    };
    let ack_ticks = (world.tick + 1).saturating_sub(c.seq_start_tick).min(255) as u8;
    let mut out = Vec::with_capacity(64 + 9 * tanks.len() + 3 * bullets.len());
    encode_snapshot(&mut out, world.tick as u32, &own, c.last_seq, ack_ticks, &world.bounds, &tanks, &bullets);
    c.out.snapshot.send_replace(out.into());
}

/// Blocks the calling thread, ticking every `TICK_MS` until `Shutdown` or all senders are gone.
pub fn run(mut game: Game, rx: std::sync::mpsc::Receiver<Inbound>) {
    use std::sync::mpsc::TryRecvError;
    let tick = Duration::from_millis(TICK_MS);
    let mut next = Instant::now() + tick;
    let (mut sum, mut max, mut n) = (Duration::ZERO, Duration::ZERO, 0u32);
    loop {
        loop {
            match rx.try_recv() {
                Ok(Inbound::Shutdown) | Err(TryRecvError::Disconnected) => {
                    game.shutdown();
                    return;
                }
                Ok(ev) => game.handle(ev),
                Err(TryRecvError::Empty) => break,
            }
        }
        let t0 = Instant::now();
        game.tick();
        let dt = t0.elapsed();
        if dt > tick {
            tracing::warn!("slow tick: {dt:?}");
        }
        sum += dt;
        max = max.max(dt);
        n += 1;
        if n == 200 {
            tracing::info!(
                "online {} players {} | tick mean {:?} max {:?}",
                game.client_count(),
                game.world.tank_count(),
                sum / n,
                max
            );
            (sum, max, n) = (Duration::ZERO, Duration::ZERO, 0);
        }
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
            next += tick;
        } else {
            next = now + tick;
        }
    }
}

/// Today's UTC date as `YYYY-MM-DD`.
fn utc_date() -> String {
    let days = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0) as i64;
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Dir;

    fn game(dir: &tempfile::TempDir, max_players: usize) -> Game {
        let cfg = GameConfig { records_path: dir.path().join("records.json"), max_players, seed: 3 };
        Game::new(cfg, Arc::new(WallMap::generate(MAP_SEED)))
    }

    fn connect(g: &mut Game, conn: u64, cap: usize) -> (watch::Receiver<Bytes>, mpsc::Receiver<Bytes>) {
        let (stx, srx) = watch::channel(Bytes::new());
        let (rtx, rrx) = mpsc::channel(cap);
        g.handle(Inbound::Connect { conn, out: Outbox { snapshot: stx, reliable: rtx } });
        (srx, rrx)
    }

    fn join(g: &mut Game, conn: u64, name: &str) {
        g.handle(Inbound::Msg { conn, msg: ClientMsg::Join { name: name.into() } });
    }

    fn input(g: &mut Game, conn: u64, seq: u16) {
        g.handle(Inbound::Msg { conn, msg: ClientMsg::Input { seq, held: None, fire: false } });
    }

    fn drain(rx: &mut mpsc::Receiver<Bytes>) -> Vec<ServerMsg> {
        let mut v = Vec::new();
        while let Ok(b) = rx.try_recv() {
            v.push(decode_server(&b).unwrap());
        }
        v
    }

    fn snapshot(rx: &watch::Receiver<Bytes>) -> SnapshotMsg {
        match decode_server(&rx.borrow()).unwrap() {
            ServerMsg::Snapshot(s) => s,
            other => panic!("not a snapshot: {other:?}"),
        }
    }

    #[test]
    fn join_gets_welcome_then_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(&dir, 10);
        let (srx, mut rrx) = connect(&mut g, 1, 64);
        join(&mut g, 1, "A");
        g.tick();
        assert!(matches!(drain(&mut rrx).first(), Some(ServerMsg::Welcome { .. })));
        let s = snapshot(&srx);
        assert!(s.bounds.contains_box(s.own.x as i32, s.own.y as i32));
    }

    #[test]
    fn input_before_join_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(&dir, 10);
        let (_s, _r) = connect(&mut g, 1, 64);
        input(&mut g, 1, 1);
        g.tick();
        assert_eq!(g.world.tank_count(), 0);
    }

    #[test]
    fn join_while_alive_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(&dir, 10);
        let (_s, mut rrx) = connect(&mut g, 1, 64);
        join(&mut g, 1, "A");
        join(&mut g, 1, "A");
        assert_eq!(g.world.tank_count(), 1);
        assert_eq!(drain(&mut rrx).iter().filter(|m| matches!(m, ServerMsg::Welcome { .. })).count(), 1);
    }

    #[test]
    fn names_sent_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(&dir, 10);
        let (_sa, mut ra) = connect(&mut g, 1, 64);
        let (_sb, _rb) = connect(&mut g, 2, 64);
        let a = g.world.place("A".into(), 1000, 1000, Dir::Up);
        let b = g.world.place("B".into(), 1010, 1000, Dir::Up);
        g.clients.get_mut(&1).unwrap().tank = Some(a);
        g.clients.get_mut(&2).unwrap().tank = Some(b);
        for _ in 0..5 {
            g.tick();
        }
        let names: Vec<_> = drain(&mut ra).into_iter().filter_map(|m| match m {
            ServerMsg::Names(n) => Some(n),
            _ => None,
        }).collect();
        assert_eq!(names, vec![vec![(b, "B".to_string())]]);
    }

    #[test]
    fn ack_counts_ticks() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(&dir, 10);
        let (srx, _r) = connect(&mut g, 1, 64);
        join(&mut g, 1, "A");
        g.tick();
        input(&mut g, 1, 7);
        for _ in 0..3 {
            g.tick();
        }
        let s = snapshot(&srx);
        assert_eq!((s.ack_seq, s.ack_ticks), (7, 3));
    }

    #[test]
    fn death_sends_dead_and_records() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(&dir, 10);
        let (_s, mut rrx) = connect(&mut g, 1, 64);
        let (x, y) = crate::world::tests::free_strip(&g.world, 20, 3);
        let s = g.world.place("S".into(), x, y, Dir::Right);
        let v = g.world.place("V".into(), x + 13, y, Dir::Left);
        g.world.set_kills(v, 3);
        g.clients.get_mut(&1).unwrap().tank = Some(v);
        g.world.set_input(s, None, true);
        for _ in 0..15 {
            g.world.set_input(v, None, false);
            g.tick();
        }
        let dead = drain(&mut rrx).into_iter().find(|m| matches!(m, ServerMsg::Dead { .. }));
        match dead {
            Some(ServerMsg::Dead { cause, killer, kills, record_flags, .. }) => {
                assert_eq!((cause, killer.as_str(), kills, record_flags & 1), (0, "S", 3, 1));
            }
            other => panic!("no DEAD: {other:?}"),
        }
        let saved = std::fs::read_to_string(dir.path().join("records.json")).unwrap();
        assert!(saved.contains("\"V\""));
        assert!(g.clients[&1].tank.is_none());
    }

    #[test]
    fn full_reliable_queue_drops_client() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(&dir, 10);
        let (_sa, _ra) = connect(&mut g, 1, 1);
        let (sb, mut rb) = connect(&mut g, 2, 64);
        join(&mut g, 1, "A");
        join(&mut g, 2, "B");
        for _ in 0..BOARD_EVERY {
            g.tick();
            drain(&mut rb);
        }
        assert_eq!(g.client_count(), 1);
        assert_eq!(g.world.tank_count(), 1);
        let before = snapshot(&sb).tick;
        g.tick();
        assert_eq!(snapshot(&sb).tick, before + 1);
    }

    #[test]
    fn server_full() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(&dir, 1);
        let (_sa, _ra) = connect(&mut g, 1, 64);
        let (_sb, mut rb) = connect(&mut g, 2, 64);
        join(&mut g, 1, "A");
        join(&mut g, 2, "B");
        assert_eq!(drain(&mut rb), vec![ServerMsg::Full]);
        assert_eq!(g.world.tank_count(), 1);
    }

    #[test]
    fn input_flood_drops_client() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(&dir, 10);
        let (_s, _r) = connect(&mut g, 1, 64);
        join(&mut g, 1, "A");
        for seq in 0..INPUT_RATE_MAX as u16 {
            input(&mut g, 1, seq);
        }
        assert_eq!(g.client_count(), 1);
        input(&mut g, 1, 999);
        assert_eq!(g.client_count(), 0);
        assert_eq!(g.world.tank_count(), 0);
    }
}
