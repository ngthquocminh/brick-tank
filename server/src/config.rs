//! Every tunable constant of the game, in one place.

pub const MAP_SIZE: i32 = 2048;
pub const BLOCK: i32 = 18;
pub const BLOCK_MARGIN: i32 = 2;
pub const VIEW: i32 = 36;
pub const VIEW_HALF: i32 = 17;
pub const VIEW_MARGIN: i32 = 4;
pub const DX_OFFSET: i32 = 8;
pub const TICK_MS: u64 = 50;
pub const MOVE_COOLDOWN: u8 = 1;
pub const BULLET_RANGE: u8 = 25;
pub const INVULN_TICKS: u64 = 60;
pub const AFK_WARN_TICKS: u64 = 1000;
pub const AFK_KILL_TICKS: u64 = 1200;
pub const AREA_PER_PLAYER: i64 = 400;
pub const S_MIN: i32 = 60;
pub const SHRINK_SLACK: f64 = 1.15;
pub const SHRINK_EVERY: u64 = 40;
pub const SPAWN_CANDIDATES: usize = 16;
pub const SPAWN_SAFE_DIST: i32 = 20;
pub const BOARD_ALIVE_N: usize = 20;
pub const RECORDS_N: usize = 5;
pub const NAME_MAX: usize = 10;
pub const MAP_SEED: u64 = 0xB41C_7A4E;
pub const BOARD_EVERY: u64 = 20;
pub const INPUT_RATE_MAX: u32 = 60;
pub const RELIABLE_QUEUE: usize = 64;
