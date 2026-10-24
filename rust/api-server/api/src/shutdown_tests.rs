use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn axum_shutdown_finishes_an_in_flight_request() {
    tokio::time::timeout(Duration::from_secs(3), async {
        let stop = tokio_util::sync::CancellationToken::new();
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let app = Router::new().route(
            "/",
            axum::routing::get({
                let entered = entered.clone();
                let release = release.clone();
                move || {
                    let entered = entered.clone();
                    let release = release.clone();
                    async move {
                        entered.notify_one();
                        release.notified().await;
                        "finished"
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let graceful = stop.clone();
        let mut server = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(graceful.cancelled_owned())
                .await
        });
        let mut client = tokio::net::TcpStream::connect(address).await.unwrap();
        client
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        entered.notified().await;
        stop.cancel();
        assert!(futures::poll!(&mut server).is_pending());
        release.notify_one();
        let mut response = String::new();
        client.read_to_string(&mut response).await.unwrap();
        assert!(response.contains("200 OK"));
        assert!(response.ends_with("finished"));
        server.await.unwrap().unwrap();
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn websocket_shutdown_closes_socket_and_releases_client_state() {
    tokio::time::timeout(Duration::from_secs(4), async {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(DatabaseConnection::default(), tx, Default::default());
        let shutdown = state.shutdown.clone();
        let app = Router::new().route("/ws", axum::routing::get(websocket_handler)).with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let stop = shutdown.stop.clone();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).with_graceful_shutdown(stop.cancelled_owned()).await
        });
        let mut client = tokio::net::TcpStream::connect(address).await.unwrap();
        client.write_all(b"GET /ws HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n").await.unwrap();
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            header.push(client.read_u8().await.unwrap());
        }
        assert!(String::from_utf8_lossy(&header).contains("101 Switching Protocols"));
        while state.clients_state.clients.read().await.is_empty() { tokio::task::yield_now().await; }
        shutdown.drain().await;
        server.await.unwrap().unwrap();
        assert!(state.clients_state.clients.read().await.is_empty());
        let mut frames = Vec::new();
        client.read_to_end(&mut frames).await.unwrap();
        // Empty server-side Close frame (FIN + opcode 8, zero payload).
        assert!(frames.ends_with(&[0x88, 0x00]));
        state.ranking_system.shutdown().await;
    }).await.unwrap();
}

#[test]
fn six_minute_limit_is_an_explicit_serve_option() {
    let cli = Cli::try_parse_from(["api", "serve", "--shutdown-after-seconds", "360"]).unwrap();
    assert!(matches!(
        cli.command,
        Commands::Serve {
            shutdown_after_seconds: Some(360),
            ..
        }
    ));
    let cli = Cli::try_parse_from(["api", "serve"]).unwrap();
    assert!(matches!(
        cli.command,
        Commands::Serve {
            shutdown_after_seconds: None,
            ..
        }
    ));
    assert!(Cli::try_parse_from(["api", "serve", "--shutdown-after-seconds", "-1"]).is_err());
}
