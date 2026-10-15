//! DCS integration without DCS: fake Lua export / GameGUI / command datagrams
//! on test ports, checked through the real listeners and sync thread.

use std::net::UdpSocket;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use srs_client::dcs_io::{self, DcsPorts};
use srs_client::dcs_sync::{run_dcs_sync, DcsSyncCtx, NetSink};
use srs_client::radio_state::shared_radio_state;
use srs_client::{AppEvent, ClientState, MergeContext};
use srs_net::{NetCommand, SharedState};
use srs_protocol::Modulation;

const EXPORT: &str = r#"{"name":"Maverick","latLng":{"lat":43.1,"lng":41.2,"alt":1000.0},"ptt":false,
 "radios":[{"name":"Intercom","freq":100.0,"modulation":2,"volume":1.0},
           {"name":"COMM1","freq":251000000.0,"modulation":0,"freqMin":225000000.0,"freqMax":400000000.0,"freqMode":0,"volMode":0,"volume":0.8},
           {"name":"COMM2","freq":124000000.0,"modulation":0,"freqMin":118000000.0,"freqMax":136000000.0,"freqMode":1,"volMode":1,"volume":1.0}],
 "selected":1,"seat":0,"unit":"FA-18C_hornet","unitId":1.6777216e+07,
 "iff":{"control":0,"mode1":51,"mode3":7700,"mode4":true,"status":1},"ambient":{"abType":"fa18","vol":0.15},
 "capabilities":{"dcsPtt":true,"dcsIFF":true,"dcsRadioSwitch":true,"intercomHotMic":false,"desc":""},
 "control":0,"inAircraft":true,"intercomHotMic":false,"simultaneousTransmission":false,"simultaneousTransmissionControl":0}"#;

#[test]
fn export_gamegui_and_commands_flow_through() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).try_init();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let ports = DcsPorts {
        export_in: 29084,
        gamegui_in: 25068,
        los_in: 29085,
        los_out: 29086,
        autoconnect_in: 25069,
        command_in: 29040,
        info_out: 27080,
        other_out: 27082,
    };
    // Overlay sink (7080 equivalent).
    let overlay = UdpSocket::bind("127.0.0.1:27080").unwrap();
    overlay.set_read_timeout(Some(Duration::from_secs(3))).unwrap();

    let state = ClientState::new("init".into());
    let guid = state.guid.to_string();
    let shared = Arc::new(SharedState::new());
    let radio_state = shared_radio_state();
    let (dcs_tx, dcs_rx) = mpsc::channel();
    let (app_tx, app_rx) = mpsc::channel::<AppEvent>();
    let (net_tx, net_rx) = mpsc::channel::<NetCommand>();
    let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
    let io = Arc::new(dcs_io::spawn(
        rt.handle(),
        ports,
        dcs_tx,
        wake.clone(),
        shared.clone(),
        radio_state.clone(),
        guid.clone(),
    ));
    let net: NetSink = Arc::new(move |c| {
        let _ = net_tx.send(c);
    });
    let ctx = DcsSyncCtx {
        radio_state: radio_state.clone(),
        shared: shared.clone(),
        state: Arc::new(Mutex::new(state)),
        merge_ctx: Arc::new(Mutex::new(MergeContext::default())),
        net,
        dcs_io: io.clone(),
        app_events: app_tx,
        wake,
        rotary_style: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        fixed_channels: Arc::new(Mutex::new(vec![Vec::new(); 11])),
    };
    let sync = std::thread::spawn(move || run_dcs_sync(dcs_rx, ctx));
    std::thread::sleep(Duration::from_millis(300)); // listeners bind

    let lua = UdpSocket::bind("127.0.0.1:0").unwrap();
    lua.send_to(EXPORT.as_bytes(), "127.0.0.1:29084").unwrap();

    // RADIO_UPDATE emitted with the merged radios.
    let cmd = net_rx.recv_timeout(Duration::from_secs(3)).expect("RADIO_UPDATE");
    match cmd {
        NetCommand::SendRadioUpdate(c) => {
            assert_eq!(c.name, "Maverick");
            let ri = c.radio_info.unwrap();
            assert_eq!(ri.unit, "FA-18C_hornet");
            assert_eq!(ri.unit_id, 16_777_216);
            assert_eq!(ri.radios[1].freq, 251e6);
            assert_eq!(ri.radios[1].modulation, Modulation::Am);
        }
        other => panic!("unexpected {other:?}"),
    }
    assert!(
        matches!(app_rx.recv_timeout(Duration::from_secs(1)), Ok(AppEvent::LastSeenNameChanged(n)) if n == "Maverick")
    );
    {
        let rs = radio_state.lock();
        assert!(rs.dcs.is_current());
        assert_eq!(rs.side.position.lat, 43.1);
        assert_eq!(rs.dcs.radios[2].name, "COMM2");
    }
    // Overlay line.
    let mut buf = [0u8; 65536];
    let (n, _) = overlay.recv_from(&mut buf).expect("overlay state line");
    let v: serde_json::Value = serde_json::from_slice(&buf[..n - 1]).unwrap();
    assert_eq!(v["RadioInfo"]["unit"], "FA-18C_hornet");
    assert_eq!(v["TunedClients"].as_array().unwrap().len(), 11);

    // Same export again: no change → no RADIO_UPDATE within 60 s.
    lua.send_to(EXPORT.as_bytes(), "127.0.0.1:29084").unwrap();
    if let Ok(c) = net_rx.recv_timeout(Duration::from_millis(700)) {
        panic!("unchanged export must not resend, got {c:?}");
    }

    // UDP command: set COMM2 (overlay-tuned) to 130.5 MHz → forced resync on next export.
    lua.send_to(
        br#"{"Command":12,"RadioId":2,"Frequency":130.5}"#,
        "127.0.0.1:29040",
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(radio_state.lock().dcs.radios[2].freq, 130.5e6);
    lua.send_to(EXPORT.as_bytes(), "127.0.0.1:29084").unwrap();
    let cmd = net_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("RADIO_UPDATE after command");
    match cmd {
        NetCommand::SendRadioUpdate(c) => assert_eq!(c.radio_info.unwrap().radios[2].freq, 130.5e6),
        other => panic!("unexpected {other:?}"),
    }

    // GameGUI: coalition/seat → metadata UPDATE.
    lua.send_to(br#"{"name":"Maverick","side":2,"seat":0}"#, "127.0.0.1:25068")
        .unwrap();
    match net_rx.recv_timeout(Duration::from_secs(3)).expect("UPDATE") {
        NetCommand::SendMetadata(c) => {
            assert_eq!(c.coalition, 2);
            assert_eq!(
                c.lat_lng_position.unwrap().lat,
                43.1,
                "position kept while export is fresh"
            );
        }
        other => panic!("unexpected {other:?}"),
    }

    // Auto-connect.
    lua.send_to(b"srs.example.org:5002", "127.0.0.1:25069").unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match app_rx.recv_timeout(Duration::from_millis(500)) {
            Ok(AppEvent::AutoConnect(a)) => {
                assert_eq!(a, "srs.example.org:5002");
                break;
            }
            Ok(other) => eprintln!("skipping {other:?}"),
            Err(_) => assert!(Instant::now() < deadline, "no AutoConnect event"),
        }
    }

    io.stop();
    drop(io);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !sync.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(sync.is_finished(), "sync thread should stop when listeners stop");
}
