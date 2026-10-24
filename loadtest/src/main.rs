//! Load test: `loadtest <ws-url> <bots> [ramp_per_sec]`. Bots join, wander and shoot at random,
//! rejoin after dying, and every 5 s the tool prints throughput and snapshot-gap percentiles.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use server::protocol::{decode_server, ServerMsg};
use server::rng::Rng;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Default)]
struct Stats {
    connected: AtomicU64,
    deaths: AtomicU64,
    full: AtomicU64,
    msgs: AtomicU64,
    bytes: AtomicU64,
    gaps_ms: Mutex<Vec<u32>>,
}

fn join_msg(name: &str) -> Message {
    let mut v = vec![0x01, name.len() as u8];
    v.extend_from_slice(name.as_bytes());
    Message::Binary(Bytes::from(v))
}

async fn bot(url: String, n: u64, stats: Arc<Stats>) {
    let Ok((ws, _)) = connect_async(&url).await else { return };
    stats.connected.fetch_add(1, Relaxed);
    let (mut tx, mut rx) = ws.split();
    let name = format!("BOT{}", n % 10_000_000);
    let mut rng = Rng::new(n.wrapping_mul(0x9E37_79B9) + 1);
    let _ = tx.send(join_msg(&name)).await;
    let (mut seq, mut last_snap) = (0u16, None::<Instant>);
    let mut next_input = tokio::time::Instant::now();
    let mut rejoin_at: Option<tokio::time::Instant> = None;
    loop {
        let wake = rejoin_at.unwrap_or(next_input);
        tokio::select! {
            m = rx.next() => {
                let Some(Ok(Message::Binary(b))) = m else { break };
                stats.msgs.fetch_add(1, Relaxed);
                stats.bytes.fetch_add(b.len() as u64, Relaxed);
                match decode_server(&b) {
                    Ok(ServerMsg::Snapshot(_)) => {
                        let now = Instant::now();
                        if let Some(prev) = last_snap {
                            stats.gaps_ms.lock().unwrap().push((now - prev).as_millis() as u32);
                        }
                        last_snap = Some(now);
                    }
                    Ok(ServerMsg::Dead { .. }) => {
                        stats.deaths.fetch_add(1, Relaxed);
                        last_snap = None;
                        rejoin_at = Some(tokio::time::Instant::now() + Duration::from_secs(1));
                    }
                    Ok(ServerMsg::Full) => {
                        stats.full.fetch_add(1, Relaxed);
                        rejoin_at = Some(tokio::time::Instant::now() + Duration::from_secs(5));
                    }
                    _ => {}
                }
            }
            _ = tokio::time::sleep_until(wake) => {
                if rejoin_at.take().is_some() {
                    if tx.send(join_msg(&name)).await.is_err() { break; }
                    continue;
                }
                seq = seq.wrapping_add(1);
                let held = rng.range(0, 5) as u8;
                let fire = rng.chance(40) as u8;
                let msg = [0x02, seq as u8, (seq >> 8) as u8, held | fire << 3];
                if tx.send(Message::Binary(Bytes::copy_from_slice(&msg))).await.is_err() { break; }
                next_input = tokio::time::Instant::now() + Duration::from_millis(rng.range(500, 2000) as u64);
            }
        }
    }
    stats.connected.fetch_sub(1, Relaxed);
}

fn pct(sorted: &[u32], p: f64) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    sorted[((sorted.len() - 1) as f64 * p) as usize]
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: loadtest <ws-url> <bots> [ramp_per_sec=200]");
        std::process::exit(2);
    }
    let url = args[1].clone();
    let bots: u64 = args[2].parse().expect("bots");
    let ramp: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(200);
    let stats = Arc::new(Stats::default());

    let s = stats.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(5));
        tick.tick().await;
        loop {
            tick.tick().await;
            let mut gaps = std::mem::take(&mut *s.gaps_ms.lock().unwrap());
            gaps.sort_unstable();
            println!(
                "bots {:5} | deaths/s {:6.1} | full {} | msgs/s {:8.0} | KB/s {:8.1} | snapshot gap p50 {:3} ms p99 {:4} ms",
                s.connected.load(Relaxed),
                s.deaths.swap(0, Relaxed) as f64 / 5.0,
                s.full.swap(0, Relaxed),
                s.msgs.swap(0, Relaxed) as f64 / 5.0,
                s.bytes.swap(0, Relaxed) as f64 / 5.0 / 1024.0,
                pct(&gaps, 0.5),
                pct(&gaps, 0.99),
            );
        }
    });

    let pause = Duration::from_micros(1_000_000 / ramp.max(1));
    for n in 0..bots {
        tokio::spawn(bot(url.clone(), n, stats.clone()));
        tokio::time::sleep(pause).await;
    }
    tokio::signal::ctrl_c().await.ok();
}
