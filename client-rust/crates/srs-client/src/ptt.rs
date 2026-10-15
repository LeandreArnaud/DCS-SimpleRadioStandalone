//! Input → radio state: port of `UDPClientAudioProcessor.PTTHandler` (PTT
//! with start/release delays, radio switches) and of the radio actions in
//! `InputDeviceManager.StartPTTListening` (frequency steps, guard, keys,
//! channels, volume, overlay toggles).

use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use srs_dcs::radio_helper as rh;
use srs_input::{BindState, InputAction, InputBinding};

use crate::client::AppEvent;
use crate::encoder::TxControls;
use crate::radio_state::SharedRadioState;

/// Profile settings read by the handler.
#[derive(Debug, Clone, Default)]
pub struct PttSettings {
    pub radio_switch_is_ptt: bool,
    pub radio_switch_is_ptt_only_when_valid: bool,
    /// ms
    pub ptt_start_delay: f32,
    /// ms
    pub ptt_release_delay: f32,
    pub rotary_style_increment: bool,
}

pub struct PttHandler {
    pub settings: parking_lot::Mutex<PttSettings>,
    radio_state: SharedRadioState,
    controls: Arc<TxControls>,
    app_events: mpsc::Sender<AppEvent>,
    fixed_channels: Arc<parking_lot::Mutex<Vec<Vec<rh::PresetChannel>>>>,
    state: parking_lot::Mutex<PttMemory>,
}

#[derive(Debug, Default)]
struct PttMemory {
    first_ptt_press: Option<Instant>,
    last_ptt_press: Option<Instant>,
}

impl PttHandler {
    pub fn new(
        radio_state: SharedRadioState,
        controls: Arc<TxControls>,
        app_events: mpsc::Sender<AppEvent>,
        fixed_channels: Arc<parking_lot::Mutex<Vec<Vec<rh::PresetChannel>>>>,
        settings: PttSettings,
    ) -> Self {
        Self {
            settings: parking_lot::Mutex::new(settings),
            radio_state,
            controls,
            app_events,
            fixed_channels,
            state: parking_lot::Mutex::new(PttMemory::default()),
        }
    }

    /// One poll tick: `PTTHandler(pressed)` then the action dispatch.
    pub fn tick(&self, states: &[BindState], actions: &[InputAction]) {
        let settings = self.settings.lock().clone();
        let mut mem = self.state.lock();
        let now = Instant::now();
        let mut ptt = false;
        let mut intercom_ptt = false;
        let mut force_sync = false;
        {
            let mut rs = self.radio_state.lock();
            let current_radio = rs.dcs.selected;
            for s in states.iter().filter(|s| s.is_active) {
                let bind = s.bind();
                let v = bind.value();
                if (InputBinding::Intercom.value()..=InputBinding::Switch10.value()).contains(&v) {
                    let radio_id = (v - 100) as usize;
                    if radio_id < rs.dcs.radios.len() {
                        if rh::select_radio(&mut rs.dcs, radio_id) {
                            if settings.radio_switch_is_ptt_only_when_valid || settings.radio_switch_is_ptt {
                                mem.last_ptt_press = Some(now);
                                ptt = true;
                            }
                        } else if settings.radio_switch_is_ptt {
                            mem.last_ptt_press = Some(now);
                            ptt = true;
                        }
                    }
                } else if bind == InputBinding::Ptt {
                    mem.last_ptt_press = Some(now);
                    ptt = true;
                } else if bind == InputBinding::IntercomPTT {
                    intercom_ptt = true;
                }
            }

            // Start delay.
            if !ptt {
                mem.first_ptt_press = None;
            }
            if ptt && mem.first_ptt_press.is_none() {
                mem.first_ptt_press = Some(now);
            }
            if ptt {
                let since = mem
                    .first_ptt_press
                    .map(|t| now.duration_since(t))
                    .unwrap_or_default();
                if since.as_secs_f32() * 1000.0 < settings.ptt_start_delay {
                    self.controls.ptt.store(false, Ordering::Relaxed);
                    mem.last_ptt_press = None;
                    self.controls.intercom_ptt.store(intercom_ptt, Ordering::Relaxed);
                    return;
                }
            }
            // Release delay (only while the selected radio did not change).
            if !ptt && settings.ptt_release_delay > 0.0 && current_radio == rs.dcs.selected {
                if let Some(t) = mem.last_ptt_press {
                    if now.duration_since(t) <= Duration::from_secs_f32(settings.ptt_release_delay / 1000.0) {
                        ptt = true;
                    }
                }
            }

            // Radio actions (edge-triggered by the tracker).
            for a in actions {
                match a {
                    InputAction::ToggleOverlay => {
                        let _ = self.app_events.send(AppEvent::ToggleOverlay);
                    }
                    InputAction::ToggleAwacsOverlay => {
                        let _ = self.app_events.send(AppEvent::ToggleAwacsOverlay);
                    }
                    InputAction::Radio(bind) => {
                        if rs.dcs.is_current() {
                            let sel = rs.dcs.selected_index().unwrap_or(0);
                            let rotary = settings.rotary_style_increment;
                            let info = &mut rs.dcs;
                            let delta_mhz = match bind {
                                InputBinding::Up100 => Some(100.0),
                                InputBinding::Up10 => Some(10.0),
                                InputBinding::Up1 => Some(1.0),
                                InputBinding::Up01 => Some(0.1),
                                InputBinding::Up001 => Some(0.01),
                                InputBinding::Up0001 => Some(0.001),
                                InputBinding::Down100 => Some(-100.0),
                                InputBinding::Down10 => Some(-10.0),
                                InputBinding::Down1 => Some(-1.0),
                                InputBinding::Down01 => Some(-0.1),
                                InputBinding::Down001 => Some(-0.01),
                                InputBinding::Down0001 => Some(-0.001),
                                _ => None,
                            };
                            force_sync |= if let Some(d) = delta_mhz {
                                rh::update_frequency(info, d, sel, true, true, rotary).0
                            } else {
                                match bind {
                                    InputBinding::ToggleGuard => rh::toggle_guard(info, sel),
                                    InputBinding::ToggleEncryption => rh::toggle_encryption(info, sel),
                                    InputBinding::NextRadio => {
                                        rh::select_next_radio(info);
                                        false
                                    }
                                    InputBinding::PreviousRadio => {
                                        rh::select_previous_radio(info);
                                        false
                                    }
                                    InputBinding::EncryptionKeyIncrease => {
                                        rh::increase_encryption_key(info, sel)
                                    }
                                    InputBinding::EncryptionKeyDecrease => {
                                        rh::decrease_encryption_key(info, sel)
                                    }
                                    InputBinding::RadioChannelUp => {
                                        let list =
                                            self.fixed_channels.lock().get(sel).cloned().unwrap_or_default();
                                        rh::channel_up(info, sel, &list)
                                    }
                                    InputBinding::RadioChannelDown => {
                                        let list =
                                            self.fixed_channels.lock().get(sel).cloned().unwrap_or_default();
                                        rh::channel_down(info, sel, &list)
                                    }
                                    InputBinding::RadioVolumeUp => {
                                        rh::volume_up(info, sel);
                                        false
                                    }
                                    InputBinding::RadioVolumeDown => {
                                        rh::volume_down(info, sel);
                                        false
                                    }
                                    // TODO(M5): TransponderIDENT / TransponderPowerToggle.
                                    _ => false,
                                }
                            };
                        }
                    }
                }
            }
            if force_sync {
                rs.force_sync = true;
            }
        }
        self.controls.intercom_ptt.store(intercom_ptt, Ordering::Relaxed);
        self.controls.ptt.store(ptt, Ordering::Relaxed);
    }
}
