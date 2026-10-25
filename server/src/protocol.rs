//! Binary wire protocol (little-endian). The byte layouts are documented in docs/protocol.md.

use bytes::Bytes;

use crate::area::Bounds;
use crate::config::NAME_MAX;
use crate::rules::{held_from_code, Dir};

pub const JOIN: u8 = 0x01;
pub const INPUT: u8 = 0x02;
pub const WELCOME: u8 = 0x10;
pub const SNAPSHOT: u8 = 0x11;
pub const NAMES: u8 = 0x12;
pub const BOARD: u8 = 0x13;
pub const DEAD: u8 = 0x14;
pub const FULL: u8 = 0x15;

#[derive(Debug, PartialEq)]
pub struct ProtoError;

#[derive(Debug, PartialEq)]
pub enum ClientMsg {
    Join { name: String },
    /// What the client did on its local tick `tick`; one is sent per client tick.
    Input { tick: u16, held: Option<Dir>, fire: bool },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OwnView {
    pub x: u16,
    pub y: u16,
    pub dir: Dir,
    pub invuln: bool,
    pub afk_warn: bool,
    pub has_bullet: bool,
    pub cd: u8,
    pub kills: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TankView {
    pub id: u32,
    pub dx: u8,
    pub dy: u8,
    pub dir: Dir,
    pub invuln: bool,
    pub kills: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BulletView {
    pub dx: u8,
    pub dy: u8,
    pub own: bool,
}

#[derive(Debug, PartialEq)]
pub struct SnapshotMsg {
    pub tick: u32,
    pub own: OwnView,
    /// Last client tick applied to the own tank; `None` until one has been since joining.
    pub ack: Option<u16>,
    pub bounds: Bounds,
    pub tanks: Vec<TankView>,
    pub bullets: Vec<BulletView>,
}

#[derive(Debug, PartialEq)]
pub struct BoardMsg {
    pub online: u16,
    pub alive_kills: Vec<(String, u16)>,
    pub alive_time: Vec<(String, u32)>,
    pub rec_kills: Vec<(String, u16)>,
    pub rec_time: Vec<(String, u32)>,
    pub my_rank_kills: u16,
    pub my_rank_time: u16,
}

#[derive(Debug, PartialEq)]
pub enum ServerMsg {
    Welcome { tank_id: u32, map_version: u32 },
    Snapshot(SnapshotMsg),
    Names(Vec<(u32, String)>),
    Board(BoardMsg),
    Dead { cause: u8, killer: String, kills: u16, secs: u32, record_flags: u8 },
    Full,
}

pub fn valid_name(s: &str) -> bool {
    (1..=NAME_MAX).contains(&s.len())
        && s.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-' | b'.'))
}

struct Rd<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ProtoError> {
        let s = self.b.get(self.i..self.i + n).ok_or(ProtoError)?;
        self.i += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, ProtoError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, ProtoError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, ProtoError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn str(&mut self) -> Result<String, ProtoError> {
        let n = self.u8()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| ProtoError)
    }
    fn done(&self) -> Result<(), ProtoError> {
        if self.i == self.b.len() { Ok(()) } else { Err(ProtoError) }
    }
}

pub fn decode_client(buf: &[u8]) -> Result<ClientMsg, ProtoError> {
    let mut r = Rd { b: buf, i: 0 };
    let msg = match r.u8()? {
        JOIN => {
            let name = r.str()?;
            if !valid_name(&name) {
                return Err(ProtoError);
            }
            ClientMsg::Join { name }
        }
        INPUT => {
            let tick = r.u16()?;
            let bits = r.u8()?;
            let held = held_from_code(bits & 0x07).map_err(|_| ProtoError)?;
            ClientMsg::Input { tick, held, fire: bits & 0x08 != 0 }
        }
        _ => return Err(ProtoError),
    };
    r.done()?;
    Ok(msg)
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    let b = &s.as_bytes()[..s.len().min(255)];
    out.push(b.len() as u8);
    out.extend_from_slice(b);
}

pub fn encode_welcome(tank_id: u32, map_version: u32) -> Bytes {
    let mut out = vec![WELCOME];
    out.extend_from_slice(&tank_id.to_le_bytes());
    out.extend_from_slice(&map_version.to_le_bytes());
    out.into()
}

#[allow(clippy::too_many_arguments)]
pub fn encode_snapshot(
    out: &mut Vec<u8>,
    tick: u32,
    own: &OwnView,
    ack: Option<u16>,
    bounds: &Bounds,
    tanks: &[TankView],
    bullets: &[BulletView],
) {
    out.push(SNAPSHOT);
    out.extend_from_slice(&tick.to_le_bytes());
    out.extend_from_slice(&own.x.to_le_bytes());
    out.extend_from_slice(&own.y.to_le_bytes());
    out.push(
        own.dir as u8
            | (own.invuln as u8) << 2
            | (own.afk_warn as u8) << 3
            | (own.has_bullet as u8) << 4
            | (ack.is_some() as u8) << 5,
    );
    out.push(own.cd);
    out.extend_from_slice(&own.kills.to_le_bytes());
    out.extend_from_slice(&ack.unwrap_or(0).to_le_bytes());
    for v in [bounds.l, bounds.t, bounds.r, bounds.b] {
        out.extend_from_slice(&(v as u16).to_le_bytes());
    }
    let tanks = &tanks[..tanks.len().min(255)];
    out.push(tanks.len() as u8);
    for t in tanks {
        out.extend_from_slice(&t.id.to_le_bytes());
        out.extend_from_slice(&[t.dx, t.dy, t.dir as u8 | (t.invuln as u8) << 2]);
        out.extend_from_slice(&t.kills.to_le_bytes());
    }
    let bullets = &bullets[..bullets.len().min(255)];
    out.push(bullets.len() as u8);
    for b in bullets {
        out.extend_from_slice(&[b.dx, b.dy, b.own as u8]);
    }
}

pub fn encode_names(names: &[(u32, &str)]) -> Bytes {
    let names = &names[..names.len().min(255)];
    let mut out = vec![NAMES, names.len() as u8];
    for (id, name) in names {
        out.extend_from_slice(&id.to_le_bytes());
        put_str(&mut out, name);
    }
    out.into()
}

/// The part of BOARD shared by every client; built once per broadcast.
pub fn encode_board_common(
    online: u16,
    alive_kills: &[(&str, u16)],
    alive_time: &[(&str, u32)],
    rec_kills: &[(&str, u16)],
    rec_time: &[(&str, u32)],
) -> Vec<u8> {
    let mut out = vec![BOARD];
    out.extend_from_slice(&online.to_le_bytes());
    put_list16(&mut out, alive_kills);
    put_list32(&mut out, alive_time);
    put_list16(&mut out, rec_kills);
    put_list32(&mut out, rec_time);
    out
}

fn put_list16(out: &mut Vec<u8>, list: &[(&str, u16)]) {
    out.push(list.len().min(255) as u8);
    for (name, v) in list.iter().take(255) {
        put_str(out, name);
        out.extend_from_slice(&v.to_le_bytes());
    }
}

fn put_list32(out: &mut Vec<u8>, list: &[(&str, u32)]) {
    out.push(list.len().min(255) as u8);
    for (name, v) in list.iter().take(255) {
        put_str(out, name);
        out.extend_from_slice(&v.to_le_bytes());
    }
}

pub fn encode_board(out: &mut Vec<u8>, common: &[u8], my_rank_kills: u16, my_rank_time: u16) {
    out.extend_from_slice(common);
    out.extend_from_slice(&my_rank_kills.to_le_bytes());
    out.extend_from_slice(&my_rank_time.to_le_bytes());
}

pub fn encode_dead(cause: u8, killer: &str, kills: u16, secs: u32, record_flags: u8) -> Bytes {
    let mut out = vec![DEAD, cause];
    put_str(&mut out, killer);
    out.extend_from_slice(&kills.to_le_bytes());
    out.extend_from_slice(&secs.to_le_bytes());
    out.push(record_flags);
    out.into()
}

pub fn encode_full() -> Bytes {
    Bytes::from_static(&[FULL])
}

pub fn decode_server(buf: &[u8]) -> Result<ServerMsg, ProtoError> {
    let mut r = Rd { b: buf, i: 0 };
    let dir = |v: u8| Dir::from_u8(v & 0x03).unwrap();
    let msg = match r.u8()? {
        WELCOME => ServerMsg::Welcome { tank_id: r.u32()?, map_version: r.u32()? },
        SNAPSHOT => {
            let tick = r.u32()?;
            let (x, y) = (r.u16()?, r.u16()?);
            let f = r.u8()?;
            let own = OwnView {
                x,
                y,
                dir: dir(f),
                invuln: f & 4 != 0,
                afk_warn: f & 8 != 0,
                has_bullet: f & 16 != 0,
                cd: r.u8()?,
                kills: r.u16()?,
            };
            let ack = r.u16()?;
            let ack = (f & 32 != 0).then_some(ack);
            let bounds = Bounds { l: r.u16()? as i32, t: r.u16()? as i32, r: r.u16()? as i32, b: r.u16()? as i32 };
            let mut tanks = Vec::new();
            for _ in 0..r.u8()? {
                let (id, dx, dy, f, kills) = (r.u32()?, r.u8()?, r.u8()?, r.u8()?, r.u16()?);
                tanks.push(TankView { id, dx, dy, dir: dir(f), invuln: f & 4 != 0, kills });
            }
            let mut bullets = Vec::new();
            for _ in 0..r.u8()? {
                let (dx, dy, f) = (r.u8()?, r.u8()?, r.u8()?);
                bullets.push(BulletView { dx, dy, own: f & 1 != 0 });
            }
            ServerMsg::Snapshot(SnapshotMsg { tick, own, ack, bounds, tanks, bullets })
        }
        NAMES => {
            let mut names = Vec::new();
            for _ in 0..r.u8()? {
                names.push((r.u32()?, r.str()?));
            }
            ServerMsg::Names(names)
        }
        BOARD => {
            let online = r.u16()?;
            let l16 = |r: &mut Rd| -> Result<Vec<(String, u16)>, ProtoError> {
                (0..r.u8()?).map(|_| Ok((r.str()?, r.u16()?))).collect()
            };
            let alive_kills = l16(&mut r)?;
            let alive_time = (0..r.u8()?).map(|_| Ok((r.str()?, r.u32()?))).collect::<Result<_, _>>()?;
            let rec_kills = l16(&mut r)?;
            let rec_time = (0..r.u8()?).map(|_| Ok((r.str()?, r.u32()?))).collect::<Result<_, _>>()?;
            ServerMsg::Board(BoardMsg {
                online,
                alive_kills,
                alive_time,
                rec_kills,
                rec_time,
                my_rank_kills: r.u16()?,
                my_rank_time: r.u16()?,
            })
        }
        DEAD => ServerMsg::Dead { cause: r.u8()?, killer: r.str()?, kills: r.u16()?, secs: r.u32()?, record_flags: r.u8()? },
        FULL => ServerMsg::Full,
        _ => return Err(ProtoError),
    };
    r.done()?;
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_golden() {
        assert_eq!(
            decode_client(&[0x02, 0x34, 0x12, 0b0000_1010]),
            Ok(ClientMsg::Input { tick: 0x1234, held: Some(Dir::Right), fire: true })
        );
    }

    #[test]
    fn join_golden() {
        assert_eq!(decode_client(&[0x01, 3, b'L', b'A', b'N']), Ok(ClientMsg::Join { name: "LAN".into() }));
    }

    #[test]
    fn rejects_bad_input() {
        let join = |s: &str| {
            let mut v = vec![0x01, s.len() as u8];
            v.extend_from_slice(s.as_bytes());
            v
        };
        for bad in [
            vec![],
            vec![0x7F],
            vec![0x02, 0x01],
            vec![0x02, 0, 0, 5],
            join("lan"),
            join("ABCDEFGHIJK"),
            join("BÉ"),
            join(""),
        ] {
            assert_eq!(decode_client(&bad), Err(ProtoError), "{bad:?}");
        }
    }

    #[test]
    fn snapshot_roundtrip() {
        let own = OwnView { x: 1000, y: 1020, dir: Dir::Left, invuln: true, afk_warn: false, has_bullet: true, cd: 1, kills: 7 };
        let bounds = Bounds { l: 900, t: 901, r: 1100, b: 1101 };
        let tanks = [
            TankView { id: 9, dx: 10, dy: 12, dir: Dir::Up, invuln: false, kills: 3 },
            TankView { id: 70000, dx: 40, dy: 4, dir: Dir::Down, invuln: true, kills: 0 },
        ];
        let bullets = [
            BulletView { dx: 8, dy: 9, own: true },
            BulletView { dx: 20, dy: 30, own: false },
            BulletView { dx: 43, dy: 43, own: false },
        ];
        for ack in [Some(0xFFFF), Some(0), None] {
            let mut out = Vec::new();
            encode_snapshot(&mut out, 123456, &own, ack, &bounds, &tanks, &bullets);
            assert_eq!(out.len(), 25 + 2 * 9 + 3 * 3);
            let expected = SnapshotMsg { tick: 123456, own, ack, bounds, tanks: tanks.to_vec(), bullets: bullets.to_vec() };
            assert_eq!(decode_server(&out), Ok(ServerMsg::Snapshot(expected)));
        }
    }

    #[test]
    fn board_and_dead_roundtrip() {
        let common = encode_board_common(42, &[("LAN", 12)], &[("HUNG.99", 340)], &[("X", 99)], &[("Y_Y", 9999)]);
        let mut out = Vec::new();
        encode_board(&mut out, &common, 3, 0);
        assert_eq!(
            decode_server(&out),
            Ok(ServerMsg::Board(BoardMsg {
                online: 42,
                alive_kills: vec![("LAN".into(), 12)],
                alive_time: vec![("HUNG.99".into(), 340)],
                rec_kills: vec![("X".into(), 99)],
                rec_time: vec![("Y_Y".into(), 9999)],
                my_rank_kills: 3,
                my_rank_time: 0,
            }))
        );
        assert_eq!(
            decode_server(&encode_dead(0, "LAN", 4, 61, 2)),
            Ok(ServerMsg::Dead { cause: 0, killer: "LAN".into(), kills: 4, secs: 61, record_flags: 2 })
        );
        assert_eq!(decode_server(&encode_welcome(5, 0xDEADBEEF)), Ok(ServerMsg::Welcome { tank_id: 5, map_version: 0xDEADBEEF }));
        assert_eq!(decode_server(&encode_names(&[(5, "A"), (6, "BB")])), Ok(ServerMsg::Names(vec![(5, "A".into()), (6, "BB".into())])));
        assert_eq!(decode_server(&encode_full()), Ok(ServerMsg::Full));
    }
}
