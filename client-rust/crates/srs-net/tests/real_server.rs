//! Integration test against a real SRS server (the C# `ServerCommandLine`
//! started with `tools/docker/server/compose.yml`). Ignored by default:
//!
//! ```sh
//! SRS_TEST_SERVER=127.0.0.1:5002 cargo test -p srs-net --test real_server -- --ignored --nocapture
//! ```
//!
//! Expects EAM enabled with blue password `blue`.

use std::sync::Arc;
use std::time::Duration;

use srs_net::{NetCommand, NetEvent, SessionConfig, SharedState, TcpSession, UdpConfig, UdpVoice, VoipEvent};
use srs_protocol::{RadioInfo, ServerSettingsKey, ShortGuid, SrClient};
use tokio::time::timeout;

async fn next(rx: &mut tokio::sync::mpsc::Receiver<NetEvent>) -> NetEvent {
    timeout(Duration::from_secs(10), rx.recv())
        .await
        .expect("event timeout")
        .expect("closed")
}

#[tokio::test]
#[ignore]
async fn sync_eam_and_udp_ping_against_real_server() {
    let Ok(server) = std::env::var("SRS_TEST_SERVER") else {
        eprintln!("SRS_TEST_SERVER not set, skipping");
        return;
    };
    let endpoint = TcpSession::resolve(&server).await.unwrap();
    let guid = ShortGuid::new_random();
    let initial = SrClient {
        client_guid: guid.to_string(),
        name: "rust-integration".into(),
        radio_info: Some(RadioInfo::default()),
        lat_lng_position: Some(Default::default()),
        ..Default::default()
    };
    let shared = Arc::new(SharedState::new());
    let (handle, mut events) = TcpSession::spawn(endpoint, initial, shared.clone(), SessionConfig::default());

    assert!(matches!(next(&mut events).await, NetEvent::Connected(_)));
    let mut synced = false;
    let mut echoed = false;
    for _ in 0..4 {
        match next(&mut events).await {
            NetEvent::Synced {
                server_version,
                client_count,
            } => {
                eprintln!("server {server_version}, {client_count} clients");
                assert!(server_version
                    .parse::<srs_protocol::ProtocolVersion>()
                    .unwrap()
                    .is_compatible());
                synced = true;
            }
            NetEvent::ClientUpdated { guid: g, full } if g == guid.as_str() && full => echoed = true,
            other => eprintln!("event {other:?}"),
        }
        if synced && echoed {
            break;
        }
    }
    assert!(synced && echoed, "SYNC reply and RADIO_UPDATE echo expected");
    assert!(shared.contains(guid.as_str().as_ref()));
    let eam_enabled = shared
        .server_settings
        .read()
        .unwrap()
        .get_bool(ServerSettingsKey::EXTERNAL_AWACS_MODE);
    assert!(eam_enabled, "test server must run with --enableEAM");
    let test_freqs = shared.server_settings.read().unwrap().test_frequencies_hz();
    eprintln!("test frequencies: {test_freqs:?}");

    // EAM login.
    handle
        .send(NetCommand::EamLogin {
            name: "rust-integration".into(),
            password: "blue".into(),
        })
        .await;
    loop {
        match next(&mut events).await {
            NetEvent::EamResult { coalition } => {
                assert_eq!(coalition, 2, "blue password must give coalition 2");
                break;
            }
            other => eprintln!("event {other:?}"),
        }
    }

    // UDP ping -> ready.
    let (udp, _voice_rx, mut voip) = UdpVoice::spawn(endpoint, guid, UdpConfig::default());
    assert_eq!(
        timeout(Duration::from_secs(10), voip.recv()).await.unwrap(),
        Some(VoipEvent::Ready(true))
    );
    assert!(udp.is_ready());
    udp.stop();

    handle.disconnect();
    loop {
        if let NetEvent::Disconnected(_) = next(&mut events).await {
            break;
        }
    }
}
