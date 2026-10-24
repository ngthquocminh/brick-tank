use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use server::config::MAP_SEED;
use server::game::{run, Game, GameConfig, Inbound};
use server::map::WallMap;
use server::net::{router, serve, NetConfig};
use tokio::net::TcpListener;

fn env<T: FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let bind: String = env("BIND", "0.0.0.0:8080".to_string());
    let static_dir: PathBuf = env("STATIC_DIR", PathBuf::from("client"));
    let records_path: PathBuf = env("RECORDS", PathBuf::from("records.json"));
    let max_players: usize = env("MAX_PLAYERS", 3000);
    let max_per_ip: u32 = env("MAX_PER_IP", 5);
    let trust_proxy = env("TRUST_PROXY", 0u8) == 1;

    let map = Arc::new(WallMap::generate(MAP_SEED));
    let map_bin = Bytes::from(map.encode());
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
    let game = Game::new(GameConfig { records_path, max_players, seed }, map);
    let (tx, rx) = std::sync::mpsc::channel();
    let game_thread = std::thread::spawn(move || run(game, rx));

    let listener = TcpListener::bind(&bind).await.expect("bind");
    tracing::info!("listening on {bind}, serving {}", static_dir.display());
    let app = router(tx.clone(), map_bin, NetConfig { static_dir, max_per_ip, trust_proxy });
    tokio::select! {
        _ = serve(listener, app) => {}
        _ = shutdown_signal() => tracing::info!("shutting down"),
    }
    let _ = tx.send(Inbound::Shutdown);
    let _ = game_thread.join();
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("signal");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}
