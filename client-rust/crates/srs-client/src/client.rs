//! The client façade: runs the tokio runtime on background threads and ties
//! together the network session, the voice transport, the decode thread, the
//! audio output and the EAM feeder (`MainWindowViewModel` + `AudioManager`
//! glue in C#). Events reach the caller through [`Client::poll`]; `wake` is
//! called whenever a new event is queued (the UI uses it to repaint).

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use srs_audio::devices::{InputStream, OutputStream};
use srs_audio::{shared_engine, MicSettings, SharedEngine};
use srs_net::{
    NetCommand, NetEvent, SessionConfig, SharedState, TcpHandle, TcpSession, UdpConfig, UdpHandle, UdpVoice,
    VoipEvent,
};
use srs_protocol::SrClient;

use parking_lot::Mutex;
use srs_dcs::merge::MergeContext;

use crate::dcs_io::{self, DcsIo, DcsPorts};
use crate::dcs_sync::{run_dcs_sync, DcsSyncCtx};
use crate::decode::{run_decode, DecodePrefs};
use crate::encoder::{run_encoder, EncoderParams, TxControls};
use crate::radio_state::{shared_radio_state, RadioState, SharedRadioState};
use crate::state::ClientState;

/// Called from background threads when an event is queued.
pub type WakeFn = Arc<dyn Fn() + Send + Sync>;

/// Everything the UI needs to react to.
#[derive(Debug, Clone)]
pub enum AppEvent {
    Net(NetEvent),
    Voip(VoipEvent),
    /// DCS asked us to connect (`host:port`, UDP 5069 or `-host=`).
    AutoConnect(String),
    /// The DCS export carried a new player name (persist `LastSeenName`).
    LastSeenNameChanged(String),
    /// New aircraft/seat (profile auto-selection).
    NewUnitEntered {
        unit: String,
        seat: i32,
    },
    /// A radio with overlay tuning appeared: load its preset channels.
    ReloadFixedChannels {
        radio: usize,
        min: f64,
        max: f64,
    },
    /// `OverlayToggle` binding pressed (debounce in the UI).
    ToggleOverlay,
    /// `AwacsOverlayToggle` binding pressed.
    ToggleAwacsOverlay,
    /// `assign_button` result: the user pressed this input.
    ButtonAssigned(srs_input::InputDevice),
}

pub struct Client {
    rt: tokio::runtime::Runtime,
    pub shared: Arc<SharedState>,
    pub engine: SharedEngine,
    pub radio_state: SharedRadioState,
    pub prefs: Arc<DecodePrefs>,
    pub tx: Arc<TxControls>,
    pub mic_settings: MicSettings,
    /// Identity and metadata (shared with the DCS sync thread).
    pub state: Arc<Mutex<ClientState>>,
    /// Settings-derived merge rules, set by the UI.
    pub merge_ctx: Arc<Mutex<MergeContext>>,
    pub rotary_style: Arc<AtomicBool>,
    /// Preset channel lists per radio (index = radio id).
    pub fixed_channels: Arc<Mutex<Vec<Vec<srs_dcs::radio_helper::PresetChannel>>>>,
    pub dcs_ports: DcsPorts,
    /// PTT / radio action handler fed by the input poller.
    pub ptt: Arc<crate::ptt::PttHandler>,
    input_poller: Option<srs_input::PollerHandle>,
    dcs_io: Option<Arc<DcsIo>>,
    dcs_thread: Option<std::thread::JoinHandle<()>>,
    handle: Option<TcpHandle>,
    input: Option<InputStream>,
    encoder_thread: Option<std::thread::JoinHandle<()>>,
    mic_settings_tx: Option<mpsc::Sender<MicSettings>>,
    udp: Option<UdpHandle>,
    output: Option<OutputStream>,
    decode_tx: Option<mpsc::Sender<Vec<u8>>>,
    decode_thread: Option<std::thread::JoinHandle<()>>,
    eam_running: Option<Arc<AtomicBool>>,
    endpoint: Option<SocketAddr>,
    events_rx: mpsc::Receiver<AppEvent>,
    events_tx: mpsc::Sender<AppEvent>,
    wake: WakeFn,
    /// Open the output device in `start_audio` (off for headless tests).
    pub use_output_device: bool,
    /// cpal device names (None/empty = system default), from
    /// `AudioInputDeviceId` / `AudioOutputDeviceId`.
    pub audio_input_device: Option<String>,
    pub audio_output_device: Option<String>,
}

impl Client {
    pub fn new(wake: WakeFn, state: ClientState) -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("srs-net")
            .build()
            .expect("tokio runtime");
        let (events_tx, events_rx) = mpsc::channel();
        let radio_state = shared_radio_state();
        let tx = Arc::new(TxControls::default());
        let fixed_channels = Arc::new(Mutex::new(vec![Vec::new(); srs_protocol::constants::MAX_RADIOS]));
        let ptt = Arc::new(crate::ptt::PttHandler::new(
            radio_state.clone(),
            tx.clone(),
            events_tx.clone(),
            fixed_channels.clone(),
            crate::ptt::PttSettings::default(),
        ));
        Self {
            rt,
            shared: Arc::new(SharedState::new()),
            engine: shared_engine(),
            radio_state,
            prefs: Arc::new(DecodePrefs {
                show_transmitter_name: AtomicBool::new(true),
            }),
            tx,
            mic_settings: MicSettings::default(),
            state: Arc::new(Mutex::new(state)),
            merge_ctx: Arc::new(Mutex::new(MergeContext::default())),
            rotary_style: Arc::new(AtomicBool::new(false)),
            fixed_channels,
            dcs_ports: DcsPorts::default(),
            ptt,
            input_poller: None,
            dcs_io: None,
            dcs_thread: None,
            handle: None,
            input: None,
            encoder_thread: None,
            mic_settings_tx: None,
            udp: None,
            output: None,
            decode_tx: None,
            decode_thread: None,
            eam_running: None,
            endpoint: None,
            events_rx,
            events_tx,
            wake,
            use_output_device: true,
            audio_input_device: None,
            audio_output_device: None,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.handle.is_some()
    }

    pub fn voip_ready(&self) -> bool {
        self.udp.as_ref().is_some_and(UdpHandle::is_ready)
    }

    pub fn audio_running(&self) -> bool {
        self.output.is_some()
    }

    /// Resolve and connect (TCP). Audio starts on `NetEvent::Connected`.
    pub fn connect(&mut self, address: &str, initial: SrClient, config: SessionConfig) -> Result<(), String> {
        self.disconnect();
        let shared = self.shared.clone();
        let events_tx = self.events_tx.clone();
        let wake = self.wake.clone();
        let address = address.to_string();
        let (handle, endpoint) = self.rt.block_on(async move {
            let endpoint = TcpSession::resolve(&address).await?;
            let (handle, mut rx) = TcpSession::spawn(endpoint, initial, shared, config);
            tokio::spawn(async move {
                while let Some(evt) = rx.recv().await {
                    if events_tx.send(AppEvent::Net(evt)).is_err() {
                        break;
                    }
                    wake();
                }
            });
            Ok::<(TcpHandle, SocketAddr), String>((handle, endpoint))
        })?;
        self.handle = Some(handle);
        self.endpoint = Some(endpoint);
        Ok(())
    }

    /// `StartAudio`: UDP voice + decode thread + output device.
    pub fn start_audio(&mut self, guid: srs_protocol::ShortGuid) {
        let Some(endpoint) = self.endpoint else { return };
        self.stop_audio();
        self.engine.lock().clear();

        let (decode_tx, decode_rx) = mpsc::channel::<Vec<u8>>();
        let (udp, mut voice_rx, mut voip_rx) = self
            .rt
            .block_on(async { UdpVoice::spawn(endpoint, guid, UdpConfig::default()) });
        let tx = decode_tx.clone();
        self.rt.spawn(async move {
            while let Some(datagram) = voice_rx.recv().await {
                if tx.send(datagram).is_err() {
                    break;
                }
            }
        });
        let events_tx = self.events_tx.clone();
        let wake = self.wake.clone();
        self.rt.spawn(async move {
            while let Some(evt) = voip_rx.recv().await {
                if events_tx.send(AppEvent::Voip(evt)).is_err() {
                    break;
                }
                wake();
            }
        });
        let decode_ctx = crate::decode::DecodeCtx {
            my_guid: guid,
            shared: self.shared.clone(),
            radio_state: self.radio_state.clone(),
            engine: self.engine.clone(),
            prefs: self.prefs.clone(),
            controls: self.tx.clone(),
            udp: udp.clone(),
        };
        self.decode_thread = Some(
            std::thread::Builder::new()
                .name("srs-decode".into())
                .spawn(move || run_decode(decode_rx, decode_ctx))
                .expect("decode thread"),
        );
        self.decode_tx = Some(decode_tx);
        // Microphone → encoder thread → UDP.
        if self.use_output_device {
            let (cap_tx, cap_rx) = mpsc::sync_channel::<Vec<f32>>(64);
            match InputStream::start(self.audio_input_device.as_deref(), cap_tx) {
                Ok(input) => {
                    let (ms_tx, ms_rx) = mpsc::channel();
                    let params = EncoderParams {
                        in_rate: input.sample_rate,
                        channels: input.channels,
                        guid,
                        settings: self.mic_settings.clone(),
                    };
                    let udp_for_enc = udp.clone();
                    let radio_state = self.radio_state.clone();
                    let controls = self.tx.clone();
                    controls.mic_available.store(true, Ordering::Relaxed);
                    let engine_for_enc = self.engine.clone();
                    self.encoder_thread = Some(
                        std::thread::Builder::new()
                            .name("srs-encoder".into())
                            .spawn(move || {
                                run_encoder(
                                    cap_rx,
                                    params,
                                    udp_for_enc,
                                    radio_state,
                                    controls,
                                    ms_rx,
                                    engine_for_enc,
                                )
                            })
                            .expect("encoder thread"),
                    );
                    self.mic_settings_tx = Some(ms_tx);
                    self.input = Some(input);
                }
                Err(e) => {
                    log::error!("Problem initialising Audio Input! {e}");
                    self.tx.mic_available.store(false, Ordering::Relaxed);
                }
            }
        }
        self.udp = Some(udp);
        if self.use_output_device {
            match OutputStream::start(self.engine.clone(), self.audio_output_device.as_deref()) {
                Ok(s) => self.output = Some(s),
                Err(e) => log::error!("Problem Initialising Audio Output! {e}"),
            }
        }
    }

    /// Queue an encoded voice packet for sending (tests / external feeders).
    pub fn send_voice(&self, packet: Vec<u8>) -> bool {
        self.udp.as_ref().is_some_and(|u| u.send(packet))
    }

    /// Push new microphone settings to the encoder thread.
    pub fn set_mic_settings(&mut self, settings: MicSettings) {
        self.mic_settings = settings.clone();
        if let Some(tx) = &self.mic_settings_tx {
            let _ = tx.send(settings);
        }
    }

    /// Start the encoder on a caller-provided capture source (tests).
    pub fn start_encoder_with_source(
        &mut self,
        guid: srs_protocol::ShortGuid,
        in_rate: u32,
        channels: u16,
        rx: mpsc::Receiver<Vec<f32>>,
    ) {
        let Some(udp) = self.udp.clone() else { return };
        let (ms_tx, ms_rx) = mpsc::channel();
        let params = EncoderParams {
            in_rate,
            channels,
            guid,
            settings: self.mic_settings.clone(),
        };
        let radio_state = self.radio_state.clone();
        let controls = self.tx.clone();
        controls.mic_available.store(true, Ordering::Relaxed);
        let engine = self.engine.clone();
        self.encoder_thread = Some(
            std::thread::Builder::new()
                .name("srs-encoder".into())
                .spawn(move || run_encoder(rx, params, udp, radio_state, controls, ms_rx, engine))
                .expect("encoder thread"),
        );
        self.mic_settings_tx = Some(ms_tx);
    }

    /// Start the input poller (`InputDeviceManager.StartPTTListening`). Can
    /// run before connecting; PTT only transmits once audio is up.
    pub fn start_input(
        &mut self,
        backend: Box<dyn srs_input::InputBackend>,
        bindings: std::collections::BTreeMap<srs_input::InputBinding, srs_input::InputDevice>,
    ) {
        self.stop_input();
        let ptt = self.ptt.clone();
        let wake = self.wake.clone();
        let on_tick: srs_input::poller::TickCallback = Arc::new(move |tick: srs_input::poller::PollTick| {
            ptt.tick(&tick.states, &tick.actions);
            if !tick.actions.is_empty() {
                wake();
            }
        });
        self.input_poller = Some(srs_input::Poller::start(backend, bindings, on_tick));
    }

    pub fn stop_input(&mut self) {
        if let Some(i) = self.input_poller.take() {
            i.stop();
        }
        self.tx.ptt.store(false, Ordering::Relaxed);
        self.tx.intercom_ptt.store(false, Ordering::Relaxed);
    }

    pub fn input_poller(&self) -> Option<&srs_input::PollerHandle> {
        self.input_poller.as_ref()
    }

    /// `AssignButton`: the result arrives as `AppEvent::ButtonAssigned`.
    pub fn assign_button(&self) {
        if let Some(i) = &self.input_poller {
            let tx = self.events_tx.clone();
            let wake = self.wake.clone();
            i.assign_button(Arc::new(move |d| {
                let _ = tx.send(AppEvent::ButtonAssigned(d));
                wake();
            }));
        }
    }

    /// `DCSRadioSyncManager.Start`: listeners + sync thread. Called once the
    /// TCP session is up.
    pub fn start_dcs(&mut self) {
        self.stop_dcs();
        let (ev_tx, ev_rx) = mpsc::channel();
        let guid = self.state.lock().guid.to_string();
        let io = Arc::new(dcs_io::spawn(
            self.rt.handle(),
            self.dcs_ports.clone(),
            ev_tx,
            self.wake.clone(),
            self.shared.clone(),
            self.radio_state.clone(),
            guid,
        ));
        let handle = self.handle.clone();
        let net: crate::dcs_sync::NetSink = Arc::new(move |cmd| {
            if let Some(h) = &handle {
                let _ = h.try_send(cmd);
            }
        });
        let ctx = DcsSyncCtx {
            radio_state: self.radio_state.clone(),
            shared: self.shared.clone(),
            state: self.state.clone(),
            merge_ctx: self.merge_ctx.clone(),
            net,
            dcs_io: io.clone(),
            app_events: self.events_tx.clone(),
            wake: self.wake.clone(),
            rotary_style: self.rotary_style.clone(),
            fixed_channels: self.fixed_channels.clone(),
        };
        self.dcs_thread = Some(
            std::thread::Builder::new()
                .name("srs-dcs-sync".into())
                .spawn(move || run_dcs_sync(ev_rx, ctx))
                .expect("dcs sync thread"),
        );
        self.dcs_io = Some(io);
    }

    pub fn stop_dcs(&mut self) {
        if let Some(io) = self.dcs_io.take() {
            io.stop();
        }
        // The listeners hold the event sender; once stopped they drop it and
        // the sync thread ends. Join without blocking the UI for long.
        if let Some(t) = self.dcs_thread.take() {
            let _ = t.join();
        }
    }

    pub fn dcs_running(&self) -> bool {
        self.dcs_io.is_some()
    }

    pub fn stop_audio(&mut self) {
        if let Some(u) = self.udp.take() {
            u.stop();
        }
        self.input = None; // closes the capture channel → encoder thread exits
        self.mic_settings_tx = None;
        if let Some(t) = self.encoder_thread.take() {
            let _ = t.join();
        }
        self.tx.mic_available.store(false, Ordering::Relaxed);
        self.tx.ptt.store(false, Ordering::Relaxed);
        self.radio_state.lock().sending = Default::default();
        self.output = None;
        self.decode_tx = None;
        if let Some(t) = self.decode_thread.take() {
            let _ = t.join();
        }
        self.engine.lock().clear();
    }

    pub fn disconnect(&mut self) {
        self.stop_eam();
        self.stop_dcs();
        self.stop_audio();
        if let Some(h) = self.handle.take() {
            h.disconnect();
        }
        self.endpoint = None;
        let mut rs = self.radio_state.lock();
        rs.dcs.reset();
        rs.side = Default::default();
    }

    pub fn send(&self, cmd: NetCommand) {
        if let Some(h) = &self.handle {
            if !h.try_send(cmd) {
                log::warn!("network command dropped (session gone)");
            }
        }
    }

    /// Drain pending events. Call once per UI frame.
    pub fn poll(&mut self) -> Vec<AppEvent> {
        let mut out = Vec::new();
        while let Ok(e) = self.events_rx.try_recv() {
            if let AppEvent::Net(NetEvent::Disconnected(_) | NetEvent::ConnectFailed(_)) = &e {
                self.handle = None;
                self.endpoint = None;
                self.stop_eam();
                self.stop_dcs();
                self.stop_audio();
            }
            out.push(e);
        }
        out
    }

    /// `StartExternalAWACSModeLoop`: feed synthetic radios every 200 ms and
    /// resend to the server on change or every 60 s.
    pub fn start_eam(&mut self, coalition: i32, radios: Vec<srs_dcs::DcsRadio>) {
        self.stop_eam();
        let name = self.state.lock().last_seen_name.clone();
        {
            let mut rs = self.radio_state.lock();
            rs.eam_active = true;
            rs.intercom_offset = 1;
            rs.side.side = coalition;
            rs.side.seat = 0;
            rs.side.position = Default::default();
            let offset = rs.intercom_offset;
            rs.dcs = crate::eam::synthetic_info(&name, radios, offset);
            rs.dcs.touch();
        }
        let running = Arc::new(AtomicBool::new(true));
        self.eam_running = Some(running.clone());
        let radio_state = self.radio_state.clone();
        let handle = self.handle.clone();
        let state = self.state.clone();
        std::thread::Builder::new()
            .name("srs-eam".into())
            .spawn(move || {
                let mut last_sent: Option<Instant> = None;
                let mut last_snapshot: Option<srs_dcs::DcsPlayerRadioInfo> = None;
                while running.load(Ordering::Relaxed) {
                    let client = {
                        let mut rs = radio_state.lock();
                        if !rs.eam_active {
                            break;
                        }
                        rs.side.side = coalition;
                        rs.dcs.touch();
                        let changed = last_snapshot.as_ref().is_none_or(|s| !s.same_as(&rs.dcs));
                        let force = std::mem::take(&mut rs.force_sync);
                        let stale = last_sent.is_none_or(|t| t.elapsed() > Duration::from_secs(60));
                        if changed || force || stale {
                            last_snapshot = Some(rs.dcs.clone());
                            Some(build_client(&rs, &state.lock()))
                        } else {
                            None
                        }
                    };
                    if let (Some(c), Some(h)) = (client, &handle) {
                        last_sent = Some(Instant::now());
                        let _ = h.try_send(NetCommand::SendRadioUpdate(c));
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
                log::debug!("Stopping external AWACS mode loop");
            })
            .expect("eam thread");
    }

    pub fn stop_eam(&mut self) {
        if let Some(r) = self.eam_running.take() {
            r.store(false, Ordering::Relaxed);
            let mut rs = self.radio_state.lock();
            rs.eam_active = false;
            rs.dcs.reset();
            rs.side.side = 0;
        }
    }
}

/// Full client record for RADIO_UPDATE (`DCSRadioSyncHandler.ProcessRadioInfoAsync`).
pub fn build_client(rs: &RadioState, template: &ClientState) -> SrClient {
    let mut c = SrClient {
        client_guid: template.guid.to_string(),
        coalition: rs.side.side,
        seat: rs.side.seat,
        allow_record: template.allow_record,
        radio_info: Some(rs.dcs.to_radio_info()),
        lat_lng_position: Some(rs.side.position),
        dis_entity_id: template.dis_entity_id,
        ..Default::default()
    };
    c.set_name(&template.last_seen_name);
    c
}
