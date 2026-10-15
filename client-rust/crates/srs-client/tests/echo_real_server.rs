//! End-to-end receive test against a real SRS server using its echo
//! ("test") frequencies: we transmit Opus frames on 247.2 MHz AM and expect
//! the server to send them back, the decode thread to route them to radio 1
//! and the audio engine to render non-silent audio.
//!
//! ```sh
//! SRS_TEST_SERVER=127.0.0.1:5002 cargo test -p srs-client --test echo_real_server -- --ignored --nocapture
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use srs_audio::OpusEncoder16k;
use srs_client::{AppEvent, Client, ClientState};
use srs_dcs::DcsRadio;
use srs_net::{NetEvent, SessionConfig, VoipEvent};
use srs_protocol::constants::{MAX_OPUS_FRAME_BYTES, MIC_SEGMENT_FRAMES};
use srs_protocol::{FrequencySegment, Modulation, VoicePacket};

/// Buffers unmatched events so that waiting for A then B works whatever the
/// arrival order.
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
fn echo_frequency_roundtrip_is_heard() {
    let Ok(server) = std::env::var("SRS_TEST_SERVER") else {
        eprintln!("SRS_TEST_SERVER not set, skipping");
        return;
    };
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).try_init();
    let state = ClientState::new("rust-echo".into());
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

    // EAM with a radio tuned to the first test frequency.
    let test_freq = client
        .shared
        .server_settings
        .read()
        .unwrap()
        .test_frequencies_hz()[0];
    client.send(srs_net::NetCommand::EamLogin {
        name: "rust-echo".into(),
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
    // Wait until the server echoed our RADIO_UPDATE with the new radios.
    w.wait_for(
        &mut client,
        |e| matches!(e, AppEvent::Net(NetEvent::ClientUpdated { guid, full: true }) if guid == state.guid.as_str().as_ref()),
        "radio echo",
    );
    std::thread::sleep(Duration::from_millis(300));

    // Transmit 40 ms sine frames on the test frequency.
    let mut enc = OpusEncoder16k::new().unwrap();
    let mut pcm = [0i16; MIC_SEGMENT_FRAMES];
    let mut buf = [0u8; MAX_OPUS_FRAME_BYTES];
    let unit_id = client.radio_state.lock().dcs.unit_id;
    for n in 1..=50u64 {
        for (i, s) in pcm.iter_mut().enumerate() {
            let t = (n as usize * MIC_SEGMENT_FRAMES + i) as f32 / 16000.0;
            *s = ((t * 2.0 * std::f32::consts::PI * 440.0).sin() * 12000.0) as i16;
        }
        let len = enc.encode(&pcm, &mut buf).unwrap();
        let packet = VoicePacket {
            audio: buf[..len].to_vec(),
            frequencies: vec![FrequencySegment {
                freq_hz: test_freq,
                modulation: Modulation::Am.as_u8(),
                encryption: 0,
            }],
            unit_id,
            packet_number: n,
            retransmission_count: 0,
            original_client_guid: state.guid,
            client_guid: state.guid,
        };
        assert!(client.send_voice(packet.encode().unwrap()), "UDP not ready");
        std::thread::sleep(Duration::from_millis(40));
    }
    std::thread::sleep(Duration::from_millis(500));

    // The echo must have reached the engine on radio 1.
    let receiving = client.radio_state.lock().receiving[1].clone();
    assert!(
        receiving.last_received_at.is_some(),
        "radio 1 never received the echo"
    );
    let mut engine = client.engine.lock();
    assert_eq!(engine.clients.len(), 1, "one remote client (ourselves, echoed)");
    let mut out = vec![0f32; 1920 * 2];
    let mut peak = 0f32;
    for _ in 0..40 {
        engine.render(&mut out);
        peak = peak.max(out.iter().fold(0f32, |m, s| m.max(s.abs())));
    }
    drop(engine);
    eprintln!("rendered peak {peak}");
    assert!(peak > 0.05, "echoed audio should be audible, peak {peak}");

    client.disconnect();
}
