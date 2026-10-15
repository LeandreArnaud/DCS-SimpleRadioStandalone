//! UDP transport against a fake server socket.

use std::time::Duration;

use srs_net::{UdpConfig, UdpVoice, VoipEvent};
use srs_protocol::ShortGuid;
use tokio::net::UdpSocket;
use tokio::time::timeout;

#[tokio::test]
async fn ping_ready_voice_and_watchdog() {
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server.local_addr().unwrap();
    let guid = ShortGuid::new_random();
    let config = UdpConfig {
        ping_interval: Duration::from_millis(200),
        timeout: Duration::from_millis(600),
    };
    let (handle, mut voice_rx, mut evt_rx) = UdpVoice::spawn(server_addr, guid, config);

    // First ping arrives immediately and is exactly the GUID.
    let mut buf = [0u8; 2048];
    let (n, client_addr) = timeout(Duration::from_secs(2), server.recv_from(&mut buf))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&buf[..n], guid.as_bytes());
    assert!(!handle.is_ready());
    assert!(!handle.send(vec![1; 100]), "not ready: dropped");

    // Voice before ready is ignored.
    server.send_to(&[7u8; 100], client_addr).await.unwrap();
    // Server answers the ping -> ready.
    server.send_to(guid.as_bytes(), client_addr).await.unwrap();
    assert_eq!(
        timeout(Duration::from_secs(2), evt_rx.recv()).await.unwrap(),
        Some(VoipEvent::Ready(true))
    );
    assert!(handle.is_ready());
    assert!(
        timeout(Duration::from_millis(100), voice_rx.recv())
            .await
            .is_err(),
        "pre-ready voice must be dropped"
    );

    // Voice after ready is delivered.
    server.send_to(&[9u8; 150], client_addr).await.unwrap();
    let v = timeout(Duration::from_secs(2), voice_rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(v.len(), 150);

    // Outgoing packet reaches the server.
    assert!(handle.send(vec![3u8; 80]));
    loop {
        let (n, _) = timeout(Duration::from_secs(2), server.recv_from(&mut buf))
            .await
            .unwrap()
            .unwrap();
        if n == 80 {
            break;
        }
    }

    // Watchdog: no datagram for 600 ms -> not ready, socket recreated (new port), ping re-sent.
    assert_eq!(
        timeout(Duration::from_secs(3), evt_rx.recv()).await.unwrap(),
        Some(VoipEvent::Ready(false))
    );
    assert_eq!(
        timeout(Duration::from_secs(3), evt_rx.recv()).await.unwrap(),
        Some(VoipEvent::Timeout)
    );
    let (n, new_addr) = loop {
        let (n, a) = timeout(Duration::from_secs(2), server.recv_from(&mut buf))
            .await
            .unwrap()
            .unwrap();
        if a != client_addr {
            break (n, a);
        }
    };
    assert_eq!(&buf[..n], guid.as_bytes());
    assert_ne!(new_addr, client_addr);

    handle.stop();
    assert_eq!(
        timeout(Duration::from_secs(2), evt_rx.recv()).await.unwrap(),
        Some(VoipEvent::Stopped)
    );
}
