//! HTTP + WebSocket front end: static client, `/map.bin`, and `/ws` bridged to the game thread.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;

use crate::config::{INPUT_RATE_MAX, RELIABLE_QUEUE};
use crate::game::{Inbound, Outbox};
use crate::protocol::decode_client;

pub struct NetConfig {
    pub static_dir: PathBuf,
    pub max_per_ip: u32,
    pub trust_proxy: bool,
}

#[derive(Clone)]
struct AppState {
    tx: Sender<Inbound>,
    map_bin: Bytes,
    per_ip: Arc<Mutex<HashMap<IpAddr, u32>>>,
    next_conn: Arc<AtomicU64>,
    max_per_ip: u32,
    trust_proxy: bool,
}

/// Holds one slot of an IP's connection budget; released on drop.
struct IpSlot {
    per_ip: Arc<Mutex<HashMap<IpAddr, u32>>>,
    ip: IpAddr,
}

impl Drop for IpSlot {
    fn drop(&mut self) {
        let mut m = self.per_ip.lock().unwrap();
        if let Some(n) = m.get_mut(&self.ip) {
            *n -= 1;
            if *n == 0 {
                m.remove(&self.ip);
            }
        }
    }
}

pub fn router(tx: Sender<Inbound>, map_bin: Bytes, cfg: NetConfig) -> Router {
    let state = AppState {
        tx,
        map_bin,
        per_ip: Arc::default(),
        next_conn: Arc::new(AtomicU64::new(1)),
        max_per_ip: cfg.max_per_ip,
        trust_proxy: cfg.trust_proxy,
    };
    Router::new()
        .route("/map.bin", get(map_bin_handler))
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new(cfg.static_dir))
        .layer(CompressionLayer::new())
        .with_state(state)
}

pub async fn serve(listener: TcpListener, router: Router) {
    axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
}

async fn map_bin_handler(State(st): State<AppState>) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        st.map_bin.clone(),
    )
        .into_response()
}

async fn ws_handler(
    State(st): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    let forwarded = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .and_then(|v| v.trim().parse().ok());
    let ip = match (st.trust_proxy, forwarded) {
        (true, Some(ip)) => ip,
        _ => addr.ip(),
    };
    {
        let mut m = st.per_ip.lock().unwrap();
        let n = m.entry(ip).or_insert(0);
        if *n >= st.max_per_ip {
            return StatusCode::TOO_MANY_REQUESTS.into_response();
        }
        *n += 1;
    }
    let slot = IpSlot { per_ip: st.per_ip.clone(), ip };
    // Client messages are at most a few bytes (JOIN with a 10-char name is 12).
    ws.max_message_size(64).max_frame_size(64).on_upgrade(move |socket| handle_socket(socket, st, slot))
}

async fn handle_socket(socket: WebSocket, st: AppState, _slot: IpSlot) {
    let conn = st.next_conn.fetch_add(1, Ordering::Relaxed);
    let (stx, mut srx) = watch::channel(Bytes::new());
    let (rtx, mut rrx) = mpsc::channel::<Bytes>(RELIABLE_QUEUE);
    if st.tx.send(Inbound::Connect { conn, out: Outbox { snapshot: stx, reliable: rtx } }).is_err() {
        return;
    }
    let (mut sink, mut stream) = socket.split();

    // Writer: reliable messages first, then the latest snapshot. Ends when the game drops the outbox.
    let mut writer = tokio::spawn(async move {
        loop {
            let msg = tokio::select! {
                biased;
                m = rrx.recv() => match m {
                    Some(b) => b,
                    None => break,
                },
                r = srx.changed() => {
                    if r.is_err() {
                        break;
                    }
                    let b = srx.borrow_and_update().clone();
                    if b.is_empty() {
                        continue;
                    }
                    b
                }
            };
            if sink.send(Message::Binary(msg)).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    let tx = st.tx.clone();
    let reader = async move {
        // Hard cap on every message type, before anything reaches the game thread.
        let (mut window, mut count) = (Instant::now(), 0u32);
        while let Some(Ok(msg)) = stream.next().await {
            if window.elapsed() >= Duration::from_secs(1) {
                (window, count) = (Instant::now(), 0);
            }
            count += 1;
            if count > 2 * INPUT_RATE_MAX {
                break;
            }
            match msg {
                Message::Binary(b) => match decode_client(&b) {
                    Ok(m) => {
                        if tx.send(Inbound::Msg { conn, msg: m }).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                },
                Message::Ping(_) | Message::Pong(_) => {}
                Message::Text(_) | Message::Close(_) => break,
            }
        }
    };
    tokio::select! {
        _ = reader => {}
        _ = &mut writer => {}
    }
    let _ = st.tx.send(Inbound::Disconnect { conn });
    writer.abort();
}
