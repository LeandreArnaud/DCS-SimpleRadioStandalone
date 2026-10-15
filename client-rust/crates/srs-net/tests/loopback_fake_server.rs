//! Session tests against a minimal in-process SRS server written with tokio.
//! The fake server speaks exactly the subset of the protocol the real
//! `ServerSync` uses for a client: SYNC reply, RADIO_UPDATE echo, client
//! list, EAM, VERSION_MISMATCH.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use srs_net::tcp::DisconnectReason;
use srs_net::{NetCommand, NetEvent, SessionConfig, SharedState, TcpSession};
use srs_protocol::{MessageType, NetworkMessage, RadioInfo, ServerSettingsKey, SrClient};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

struct FakeServer {
    addr: std::net::SocketAddr,
    /// Lines received from the client, decoded.
    received: mpsc::UnboundedReceiver<NetworkMessage>,
    /// Lines to push to the client (raw, newline appended).
    push: mpsc::UnboundedSender<String>,
}

async fn start_fake_server(version: &str, extra_settings: HashMap<String, String>) -> FakeServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (rx_tx, received) = mpsc::unbounded_channel();
    let (push, mut push_rx) = mpsc::unbounded_channel::<String>();
    let version = version.to_string();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (r, mut w) = stream.into_split();
        let mut lines = BufReader::new(r).lines();
        loop {
            tokio::select! {
                line = lines.next_line() => {
                    let Ok(Some(line)) = line else { break };
                    let msg: NetworkMessage = match serde_json::from_str(&line) { Ok(m) => m, Err(_) => continue };
                    // Behave like ServerSync for the handshake.
                    if msg.msg_type == MessageType::Sync {
                        let client = msg.client.clone().unwrap();
                        let mut settings: HashMap<String, String> = extra_settings.clone();
                        settings.entry("EXTERNAL_AWACS_MODE".into()).or_insert("true".into());
                        let other = SrClient { client_guid: "AAAAAAAAAAAAAAAAAAAAAB".into(), name: "Other".into(), coalition: 2, radio_info: Some(RadioInfo::default()), ..Default::default() };
                        let reply = NetworkMessage { msg_type: MessageType::Sync, clients: Some(vec![client.clone(), other]), server_settings: Some(settings), version: Some(version.clone()), ..Default::default() };
                        w.write_all(reply.to_line(&version).as_bytes()).await.unwrap();
                        // Echo as RADIO_UPDATE like the real server.
                        let echo = NetworkMessage { msg_type: MessageType::RadioUpdate, client: Some(client), version: Some(version.clone()), ..Default::default() };
                        w.write_all(echo.to_line(&version).as_bytes()).await.unwrap();
                    } else if msg.msg_type == MessageType::ExternalAwacsModePassword {
                        let mut c = msg.client.clone().unwrap();
                        c.coalition = if msg.external_awacs_mode_password.as_deref() == Some("blue") { 2 } else { 0 };
                        let reply = NetworkMessage { msg_type: MessageType::ExternalAwacsModePassword, client: Some(c), ..Default::default() };
                        w.write_all(reply.to_line(&version).as_bytes()).await.unwrap();
                    }
                    let _ = rx_tx.send(msg);
                }
                line = push_rx.recv() => {
                    match line {
                        Some(l) => { let _ = w.write_all(format!("{l}\n").as_bytes()).await; }
                        None => break,
                    }
                }
            }
        }
    });
    FakeServer { addr, received, push }
}

fn initial_client() -> SrClient {
    SrClient {
        client_guid: "ufYS_WlLVkmFPjqCgxz6GA".into(),
        name: "Rusty".into(),
        radio_info: Some(RadioInfo::default()),
        lat_lng_position: Some(Default::default()),
        ..Default::default()
    }
}

async fn next_event(rx: &mut mpsc::Receiver<NetEvent>) -> NetEvent {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("event timeout")
        .expect("channel closed")
}

#[tokio::test]
async fn sync_handshake_and_echo() {
    let mut server = start_fake_server("2.4.1.0", HashMap::new()).await;
    let shared = Arc::new(SharedState::new());
    let (handle, mut events) = TcpSession::spawn(
        server.addr,
        initial_client(),
        shared.clone(),
        SessionConfig::default(),
    );

    assert!(matches!(next_event(&mut events).await, NetEvent::Connected(_)));
    let sync = server.received.recv().await.unwrap();
    assert_eq!(sync.msg_type, MessageType::Sync);
    assert_eq!(sync.version.as_deref(), Some("2.4.1.0"));
    assert_eq!(sync.client.as_ref().unwrap().name, "Rusty");
    assert_eq!(sync.client.as_ref().unwrap().coalition, 0);

    match next_event(&mut events).await {
        NetEvent::Synced {
            server_version,
            client_count,
        } => {
            assert_eq!(server_version, "2.4.1.0");
            assert_eq!(client_count, 2);
        }
        other => panic!("unexpected {other:?}"),
    }
    assert!(shared.contains("ufYS_WlLVkmFPjqCgxz6GA"));
    assert!(shared.contains("AAAAAAAAAAAAAAAAAAAAAB"));
    assert!(shared
        .server_settings
        .read()
        .unwrap()
        .get_bool(ServerSettingsKey::EXTERNAL_AWACS_MODE));
    assert!(matches!(
        next_event(&mut events).await,
        NetEvent::ClientUpdated { full: true, .. }
    ));

    // Metadata update: coalition change is sent, identical metadata is not.
    let mut meta = initial_client();
    meta.coalition = 2;
    handle.send(NetCommand::SendMetadata(meta.clone())).await;
    let upd = server.received.recv().await.unwrap();
    assert_eq!(upd.msg_type, MessageType::Update);
    assert!(
        upd.client.as_ref().unwrap().radio_info.is_none(),
        "UPDATE carries no RadioInfo"
    );
    handle.send(NetCommand::SendMetadata(meta.clone())).await;
    handle.send(NetCommand::SendRadioUpdate(meta.clone())).await;
    let next = server.received.recv().await.unwrap();
    assert_eq!(
        next.msg_type,
        MessageType::RadioUpdate,
        "unchanged metadata must not be sent"
    );
    assert!(next.client.as_ref().unwrap().radio_info.is_some());
    // Position zeroed because DISTANCE/LOS are off.
    assert_eq!(next.client.as_ref().unwrap().lat_lng_position.unwrap().lat, 0.0);

    // EAM.
    handle
        .send(NetCommand::EamLogin {
            name: "Overlord".into(),
            password: "blue".into(),
        })
        .await;
    let eam = server.received.recv().await.unwrap();
    assert_eq!(eam.msg_type, MessageType::ExternalAwacsModePassword);
    assert_eq!(eam.external_awacs_mode_password.as_deref(), Some("blue"));
    assert_eq!(eam.client.as_ref().unwrap().name, "Overlord");
    assert!(matches!(
        next_event(&mut events).await,
        NetEvent::EamResult { coalition: 2 }
    ));

    // Client disconnect broadcast.
    server
        .push
        .send(r#"{"Client":{"ClientGuid":"AAAAAAAAAAAAAAAAAAAAAB"},"MsgType":5}"#.into())
        .unwrap();
    assert!(
        matches!(next_event(&mut events).await, NetEvent::ClientRemoved(g) if g == "AAAAAAAAAAAAAAAAAAAAAB")
    );
    assert!(!shared.contains("AAAAAAAAAAAAAAAAAAAAAB"));

    handle.disconnect();
    assert!(matches!(
        next_event(&mut events).await,
        NetEvent::Disconnected(DisconnectReason::Requested)
    ));
    assert_eq!(shared.client_count(), 0);
}

#[tokio::test]
async fn old_server_is_rejected() {
    let server = start_fake_server("1.8.0.0", HashMap::new()).await;
    let shared = Arc::new(SharedState::new());
    let (_handle, mut events) =
        TcpSession::spawn(server.addr, initial_client(), shared, SessionConfig::default());
    assert!(matches!(next_event(&mut events).await, NetEvent::Connected(_)));
    match next_event(&mut events).await {
        NetEvent::Disconnected(DisconnectReason::VersionMismatch { server_version }) => {
            assert_eq!(server_version, "1.8.0.0")
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn version_mismatch_message_disconnects() {
    let server = start_fake_server("2.4.1.0", HashMap::new()).await;
    let shared = Arc::new(SharedState::new());
    let (_handle, mut events) =
        TcpSession::spawn(server.addr, initial_client(), shared, SessionConfig::default());
    assert!(matches!(next_event(&mut events).await, NetEvent::Connected(_)));
    assert!(matches!(next_event(&mut events).await, NetEvent::Synced { .. }));
    assert!(matches!(
        next_event(&mut events).await,
        NetEvent::ClientUpdated { .. }
    ));
    server
        .push
        .send(r#"{"MsgType":6,"Version":"9.9.9.9"}"#.into())
        .unwrap();
    assert!(matches!(
        next_event(&mut events).await,
        NetEvent::Disconnected(DisconnectReason::VersionMismatch { server_version }) if server_version == "9.9.9.9"
    ));
}

#[tokio::test]
async fn garbage_lines_disconnect_after_five() {
    let server = start_fake_server("2.4.1.0", HashMap::new()).await;
    let shared = Arc::new(SharedState::new());
    let (_handle, mut events) =
        TcpSession::spawn(server.addr, initial_client(), shared, SessionConfig::default());
    assert!(matches!(next_event(&mut events).await, NetEvent::Connected(_)));
    assert!(matches!(next_event(&mut events).await, NetEvent::Synced { .. }));
    assert!(matches!(
        next_event(&mut events).await,
        NetEvent::ClientUpdated { .. }
    ));
    for _ in 0..5 {
        server.push.send("not json".into()).unwrap();
    }
    // Five errors are tolerated; a valid line resets the counter.
    server.push.send(r#"{"MsgType":1}"#.into()).unwrap();
    for _ in 0..6 {
        server.push.send("still not json".into()).unwrap();
    }
    assert!(matches!(
        next_event(&mut events).await,
        NetEvent::Disconnected(DisconnectReason::VersionMismatch { .. })
    ));
}

#[tokio::test]
async fn idle_timeout_disconnects() {
    let server = start_fake_server("2.4.1.0", HashMap::new()).await;
    let shared = Arc::new(SharedState::new());
    let config = SessionConfig {
        idle_timeout: Duration::from_millis(300),
        idle_check_interval: Duration::from_millis(100),
        ..Default::default()
    };
    let (_handle, mut events) = TcpSession::spawn(server.addr, initial_client(), shared, config);
    assert!(matches!(next_event(&mut events).await, NetEvent::Connected(_)));
    assert!(matches!(next_event(&mut events).await, NetEvent::Synced { .. }));
    assert!(matches!(
        next_event(&mut events).await,
        NetEvent::ClientUpdated { .. }
    ));
    assert!(matches!(
        next_event(&mut events).await,
        NetEvent::Disconnected(DisconnectReason::IdleTimeout)
    ));
}

#[tokio::test]
async fn connect_failure_is_reported() {
    // Nothing listens on this port (bind then drop).
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    drop(l);
    let shared = Arc::new(SharedState::new());
    let (_handle, mut events) = TcpSession::spawn(addr, initial_client(), shared, SessionConfig::default());
    assert!(matches!(
        next_event(&mut events).await,
        NetEvent::ConnectFailed(_)
    ));
}

#[tokio::test]
async fn resolve_host_and_port() {
    let a = TcpSession::resolve("127.0.0.1").await.unwrap();
    assert_eq!(a.port(), 5002);
    let b = TcpSession::resolve(" 127.0.0.1:5010 ").await.unwrap();
    assert_eq!(b.port(), 5010);
    assert!(TcpSession::resolve("127.0.0.1:abc").await.is_err());
    assert!(TcpSession::resolve("localhost").await.is_ok());
}
