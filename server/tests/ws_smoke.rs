use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use server::config::MAP_SEED;
use server::game::{run, Game, GameConfig};
use server::map::WallMap;
use server::net::{router, serve, NetConfig};
use server::protocol::{decode_server, ServerMsg};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[tokio::test(flavor = "multi_thread")]
async fn smoke() {
    let dir = tempfile::tempdir().unwrap();
    let map = Arc::new(WallMap::generate(MAP_SEED));
    let map_bin = Bytes::from(map.encode());
    let (tx, rx) = std::sync::mpsc::channel();
    let cfg = GameConfig { records_path: dir.path().join("records.json"), max_players: 100, seed: 1 };
    let game = Game::new(cfg, map);
    std::thread::spawn(move || run(game, rx));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let net = NetConfig { static_dir: dir.path().to_path_buf(), max_per_ip: 5, trust_proxy: false };
    tokio::spawn(serve(listener, router(tx, map_bin, net)));
    let url = format!("ws://{addr}/ws");

    // JOIN → WELCOME, then SNAPSHOT.
    let (mut ws, _) = connect_async(&url).await.unwrap();
    ws.send(Message::Binary(Bytes::from_static(&[1, 3, b'B', b'O', b'T']))).await.unwrap();
    let (mut welcome, mut snap) = (false, false);
    timeout(Duration::from_secs(1), async {
        while let Some(Ok(Message::Binary(b))) = ws.next().await {
            match decode_server(&b).unwrap() {
                ServerMsg::Welcome { .. } => welcome = true,
                ServerMsg::Snapshot(_) if welcome => {
                    snap = true;
                    break;
                }
                _ => {}
            }
        }
    })
    .await
    .expect("welcome + snapshot within 1s");
    assert!(welcome && snap);

    // GET /map.bin.
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(b"GET /map.bin HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").await.unwrap();
    let mut resp = Vec::new();
    s.read_to_end(&mut resp).await.unwrap();
    let text = String::from_utf8_lossy(&resp).to_lowercase();
    assert!(text.starts_with("http/1.1 200"), "{}", &text[..40]);
    assert!(text.contains("immutable"));
    assert!(resp.windows(4).any(|w| w == b"BTMP"));

    // Per-IP limit: 5 sockets allowed, the 6th gets 429.
    let mut extra = Vec::new();
    for _ in 0..4 {
        extra.push(connect_async(&url).await.unwrap().0);
    }
    match connect_async(&url).await {
        Err(tokio_tungstenite::tungstenite::Error::Http(r)) => assert_eq!(r.status(), 429),
        other => panic!("expected 429, got {:?}", other.map(|_| ())),
    }

    // A text frame closes the socket.
    ws.send(Message::Text("hi".into())).await.unwrap();
    let closed = timeout(Duration::from_secs(1), async {
        loop {
            match ws.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return true,
                Some(Ok(_)) => {}
            }
        }
    })
    .await
    .unwrap_or(false);
    assert!(closed);
}

#[tokio::test(flavor = "multi_thread")]
async fn message_flood_closes_socket() {
    let dir = tempfile::tempdir().unwrap();
    let map = Arc::new(WallMap::generate(MAP_SEED));
    let map_bin = Bytes::from(map.encode());
    let (tx, rx) = std::sync::mpsc::channel();
    let cfg = GameConfig { records_path: dir.path().join("records.json"), max_players: 100, seed: 1 };
    std::thread::spawn(move || run(Game::new(cfg, map), rx));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let net = NetConfig { static_dir: dir.path().to_path_buf(), max_per_ip: 5, trust_proxy: false };
    tokio::spawn(serve(listener, router(tx, map_bin, net)));

    let (mut ws, _) = connect_async(format!("ws://{addr}/ws")).await.unwrap();
    for _ in 0..300 {
        if ws.send(Message::Binary(Bytes::from_static(&[1, 3, b'B', b'O', b'T']))).await.is_err() {
            break;
        }
    }
    let closed = timeout(Duration::from_secs(2), async {
        loop {
            match ws.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return true,
                Some(Ok(_)) => {}
            }
        }
    })
    .await
    .unwrap_or(false);
    assert!(closed, "JOIN flood must close the socket");
}
