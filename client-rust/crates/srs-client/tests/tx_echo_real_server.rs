//! Full TX → server echo → RX loop through the real encoder thread: a
//! synthetic 48 kHz stereo "microphone" feeds the encoder, PTT is held, the
//! server echoes our transmission on its test frequency and the engine
//! renders it back.
//!
//! ```sh
//! SRS_TEST_SERVER=127.0.0.1:5002 cargo test -p srs-client --test tx_echo_real_server -- --ignored --nocapture
//! ```

use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use srs_client::{AppEvent, Client, ClientState};
use srs_dcs::DcsRadio;
use srs_net::{NetEvent, SessionConfig, VoipEvent};
use srs_protocol::Modulation;

struct Waiter {
    pending: Vec<AppEvent>,
}

impl Waiter {
    fn wait_for<F: FnMut(&AppEvent) -> bool>(
        &mut self,
        client: &mut Client,
        mut pred: F,
        what: &str,
    ) -> AppEvent {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            // Buffer the whole batch first: an early return must not drop
            // the events that arrived together with the matching one.
            for e in client.poll() {
                eprintln!("event {e:?}");
                self.pending.push(e);
            }
            if let Some(i) = self.pending.iter().position(&mut pred) {
                return self.pending.remove(i);
            }
            assert!(Instant::now() < deadline, "timeout waiting for {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[test]
#[ignore]
fn encoder_ptt_echo_roundtrip() {
    let Ok(server) = std::env::var("SRS_TEST_SERVER") else {
        eprintln!("SRS_TEST_SERVER not set, skipping");
        return;
    };
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).try_init();
    let state = ClientState::new("rust-tx".into());
    let mut client = Client::new(Arc::new(|| {}), state.clone());
    client.use_output_device = false;
    let mut w = Waiter { pending: Vec::new() };
    client
        .connect(&server, state.initial_client(), SessionConfig::default())
        .unwrap();
    w.wait_for(
        &mut client,
        |e| matches!(e, AppEvent::Net(NetEvent::Connected(_))),
        "Connected",
    );
    client.start_audio(state.guid);
    w.wait_for(
        &mut client,
        |e| matches!(e, AppEvent::Net(NetEvent::Synced { .. })),
        "Synced",
    );
    w.wait_for(
        &mut client,
        |e| matches!(e, AppEvent::Voip(VoipEvent::Ready(true))),
        "VoIP ready",
    );

    let test_freq = client
        .shared
        .server_settings
        .read()
        .unwrap()
        .test_frequencies_hz()[0];
    client.send(srs_net::NetCommand::EamLogin {
        name: "rust-tx".into(),
        password: "blue".into(),
    });
    w.wait_for(
        &mut client,
        |e| matches!(e, AppEvent::Net(NetEvent::EamResult { coalition: 2 })),
        "EAM",
    );
    let mut radios = srs_client::eam::fallback_radios();
    radios[0] = DcsRadio {
        modulation: Modulation::Intercom,
        name: "Intercom".into(),
        freq: 100.0,
        ..Default::default()
    };
    radios[1] = DcsRadio {
        modulation: Modulation::Am,
        name: "UHF".into(),
        freq: test_freq,
        ..Default::default()
    };
    client.start_eam(2, radios);
    w.wait_for(
        &mut client,
        |e| matches!(e, AppEvent::Net(NetEvent::ClientUpdated { guid, full: true }) if guid == state.guid.as_str().as_ref()),
        "radio echo",
    );

    // Synthetic microphone: 48 kHz stereo chunks of 10 ms, 440 Hz tone.
    let (cap_tx, cap_rx) = mpsc::channel::<Vec<f32>>();
    client.start_encoder_with_source(state.guid, 48_000, 2, cap_rx);
    client.tx.ptt.store(true, Ordering::Relaxed);
    let mut t = 0usize;
    for _ in 0..150 {
        let mut chunk = Vec::with_capacity(480 * 2);
        for _ in 0..480 {
            let v = (t as f32 / 48_000.0 * 2.0 * std::f32::consts::PI * 440.0).sin() * 0.6;
            chunk.push(v);
            chunk.push(v);
            t += 1;
        }
        cap_tx.send(chunk).unwrap();
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(500));

    let sending = client.radio_state.lock().sending.clone();
    assert!(sending.is_sending, "encoder should report sending");
    assert_eq!(sending.sending_on, Some(1));
    assert!(
        client.tx.mic_level_db() > -30.0,
        "mic level {}",
        client.tx.mic_level_db()
    );
    let receiving = client.radio_state.lock().receiving[1].clone();
    assert!(
        receiving.last_received_at.is_some(),
        "echo not received on radio 1"
    );
    let mut engine = client.engine.lock();
    let mut out = vec![0f32; 1920 * 2];
    let mut peak = 0f32;
    for _ in 0..40 {
        engine.render(&mut out);
        peak = peak.max(out.iter().fold(0f32, |m, s| m.max(s.abs())));
    }
    drop(engine);
    eprintln!("rendered peak {peak}");
    assert!(peak > 0.05, "echoed audio should be audible, peak {peak}");

    client.tx.ptt.store(false, Ordering::Relaxed);
    drop(cap_tx);
    client.disconnect();
}
