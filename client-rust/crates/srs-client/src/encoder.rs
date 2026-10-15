//! Encoder thread: consumes raw capture chunks, runs the microphone pipeline
//! and transmits (`AudioManager.WasapiCaptureOnDataAvailable` +
//! `UDPClientAudioProcessor.Send`).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Instant;

use srs_audio::{MicPipeline, MicSettings, SharedEngine};
use srs_net::UdpHandle;
use srs_protocol::ShortGuid;

use crate::radio_state::{SendingState, SharedRadioState};
use crate::tx::{build_packet, ptt_pressed, TxInputs, TxMemory};

/// Lock-free inputs/outputs shared with the UI and the input poller.
pub struct TxControls {
    /// PTT from the input poller (bindings, HOTAS).
    pub ptt: AtomicBool,
    /// PTT from the UI (button / Space). OR-ed with `ptt`.
    pub ui_ptt: AtomicBool,
    pub intercom_ptt: AtomicBool,
    /// VAICOM inhibit (M4).
    pub inhibit_tx: AtomicBool,
    /// `VOXMinimumTime` in ms.
    pub vox_minimum_time_ms: AtomicU32,
    /// Microphone level in dBFS × 100 (negative), `MicMax`.
    pub mic_level_cdb: std::sync::atomic::AtomicI32,
    /// Microphone present (capture stream running).
    pub mic_available: AtomicBool,
}

impl Default for TxControls {
    fn default() -> Self {
        Self {
            ptt: AtomicBool::new(false),
            ui_ptt: AtomicBool::new(false),
            intercom_ptt: AtomicBool::new(false),
            inhibit_tx: AtomicBool::new(false),
            vox_minimum_time_ms: AtomicU32::new(300),
            mic_level_cdb: std::sync::atomic::AtomicI32::new(-10_000),
            mic_available: AtomicBool::new(false),
        }
    }
}

impl TxControls {
    pub fn mic_level_db(&self) -> f32 {
        self.mic_level_cdb.load(Ordering::Relaxed) as f32 / 100.0
    }
}

pub struct EncoderParams {
    pub in_rate: u32,
    pub channels: u16,
    pub guid: ShortGuid,
    pub settings: MicSettings,
}

/// Runs until the capture channel closes.
pub fn run_encoder(
    rx: mpsc::Receiver<Vec<f32>>,
    params: EncoderParams,
    udp: UdpHandle,
    radio_state: SharedRadioState,
    controls: Arc<TxControls>,
    settings_rx: mpsc::Receiver<MicSettings>,
    engine: SharedEngine,
) {
    let mut pipeline = match MicPipeline::new(params.in_rate, params.channels) {
        Ok(p) => p,
        Err(e) => {
            log::error!("Cannot create microphone pipeline: {e}");
            return;
        }
    };
    pipeline.set_settings(&params.settings);
    let mut mem = TxMemory::new();
    log::info!(
        "Encoder thread started ({} Hz, {} ch)",
        params.in_rate,
        params.channels
    );
    let mut packet_buf = Vec::with_capacity(1500);
    while let Ok(chunk) = rx.recv() {
        while let Ok(s) = settings_rx.try_recv() {
            pipeline.set_settings(&s);
        }
        pipeline.push_native(&chunk);
        while let Some(frame) = pipeline.next_frame() {
            controls
                .mic_level_cdb
                .store((frame.rms_db * 100.0) as i32, Ordering::Relaxed);
            let inputs = TxInputs {
                ptt: controls.ptt.load(Ordering::Relaxed) || controls.ui_ptt.load(Ordering::Relaxed),
                intercom_ptt: controls.intercom_ptt.load(Ordering::Relaxed),
                voice: frame.voice,
                vox_minimum_time_ms: u64::from(controls.vox_minimum_time_ms.load(Ordering::Relaxed)),
                inhibit_tx: controls.inhibit_tx.load(Ordering::Relaxed),
            };
            let mut state = radio_state.lock();
            let can_send =
                udp.is_ready() && state.dcs.is_current() && controls.mic_available.load(Ordering::Relaxed);
            let selection = if can_send {
                ptt_pressed(&state, &inputs, &mut mem)
            } else {
                crate::tx::TxSelection {
                    radios: Vec::new(),
                    sending_on: None,
                }
            };
            if !selection.radios.is_empty() {
                if let Some(packet) = build_packet(
                    &state,
                    &selection,
                    &frame.opus[..frame.len],
                    params.guid,
                    &mut mem,
                ) {
                    if packet.encode_into(&mut packet_buf).is_ok() {
                        udp.send(packet_buf.clone());
                    }
                    let sending_on = selection.sending_on;
                    if !state.sending.is_sending || state.sending.sending_on != sending_on {
                        // Not sending, or switched radio very quickly: start tone.
                        if let Some(radio) = sending_on.and_then(|i| state.dcs.radios.get(i)) {
                            engine.lock().play_start_transmit(
                                sending_on.unwrap_or(0),
                                radio.enc && radio.enc_key > 0,
                                radio.modulation,
                            );
                        }
                        log::debug!("TX start on radio {sending_on:?}");
                    }
                    state.sending = SendingState {
                        is_sending: true,
                        last_sent_at: Some(Instant::now()),
                        sending_on,
                    };
                }
            } else if state.sending.is_sending {
                state.sending.is_sending = false;
                if let Some(i) = state.sending.sending_on {
                    if let Some(radio) = state.dcs.radios.get(i) {
                        engine.lock().play_end_transmit(i, radio.modulation);
                    }
                }
                log::debug!("TX end");
            }
        }
    }
    log::info!("Encoder thread stopped");
}
