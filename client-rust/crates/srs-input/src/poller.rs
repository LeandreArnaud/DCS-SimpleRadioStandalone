//! The 40 ms polling thread (`StartPTTListening`).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use srs_settings::{InputBinding, InputDevice};

use crate::actions::{ActionTracker, InputAction};
use crate::backend::{DeviceSnapshot, InputBackend};
use crate::bind_state::{bind_states_from_profile, resolve_bind_states, BindState};

/// Per-tick result handed to the client (`DetectPttCallback` + actions).
pub struct PollTick {
    pub states: Vec<BindState>,
    pub actions: Vec<InputAction>,
}

pub type TickCallback = Arc<dyn Fn(PollTick) + Send + Sync>;
/// Receives the input the user pressed during `assign_button`.
pub type AssignCallback = Arc<dyn Fn(InputDevice) + Send + Sync>;

pub struct PollerHandle {
    running: Arc<AtomicBool>,
    bindings: Arc<Mutex<BTreeMap<InputBinding, InputDevice>>>,
    requests: Arc<Mutex<Vec<PollerRequest>>>,
}

enum PollerRequest {
    Rescan,
    /// `AssignButton`: deliver the first changed input to the callback.
    Assign(AssignCallback),
}

impl PollerHandle {
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    /// Replace the active profile's bindings.
    pub fn set_bindings(&self, bindings: BTreeMap<InputBinding, InputDevice>) {
        *self.bindings.lock() = bindings;
    }

    pub fn rescan(&self) {
        self.requests.lock().push(PollerRequest::Rescan);
    }

    /// Wait for the user to press something; `callback` receives it.
    pub fn assign_button(&self, callback: AssignCallback) {
        self.requests.lock().push(PollerRequest::Assign(callback));
    }
}

pub struct Poller;

impl Poller {
    pub const PERIOD: Duration = Duration::from_millis(40);

    pub fn start(
        mut backend: Box<dyn InputBackend>,
        bindings: BTreeMap<InputBinding, InputDevice>,
        on_tick: TickCallback,
    ) -> PollerHandle {
        let running = Arc::new(AtomicBool::new(true));
        let bindings = Arc::new(Mutex::new(bindings));
        let requests: Arc<Mutex<Vec<PollerRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let handle = PollerHandle {
            running: running.clone(),
            bindings: bindings.clone(),
            requests: requests.clone(),
        };
        std::thread::Builder::new()
            .name("srs-input".into())
            .spawn(move || {
                let mut tracker = ActionTracker::new();
                let mut assign: Option<(AssignCallback, Vec<DeviceSnapshot>)> = None;
                while running.load(Ordering::Relaxed) {
                    for r in requests.lock().drain(..) {
                        match r {
                            PollerRequest::Rescan => backend.rescan(),
                            PollerRequest::Assign(cb) => assign = Some((cb, backend.snapshot())),
                        }
                    }
                    if let Some((cb, initial)) = &assign {
                        if let Some(found) = detect_change(initial, &backend.snapshot()) {
                            cb(found);
                            assign = None;
                        }
                    } else {
                        let mut states = bind_states_from_profile(&bindings.lock());
                        resolve_bind_states(&mut states, |d| backend.button_state(d).unwrap_or(false));
                        let actions = tracker.actions(&states);
                        on_tick(PollTick { states, actions });
                    }
                    std::thread::sleep(Self::PERIOD);
                }
                log::info!("Input poller stopped");
            })
            .expect("input thread");
        handle
    }
}

/// `AssignButton`'s change detection: first input whose value differs from
/// the initial snapshot (buttons must be *pressed*, POV/keys just changed,
/// mouse button 0 ignored).
fn detect_change(initial: &[DeviceSnapshot], now: &[DeviceSnapshot]) -> Option<InputDevice> {
    for (a, b) in initial.iter().zip(now) {
        if a.device.instance_guid != b.device.instance_guid {
            continue;
        }
        for (j, (&before, &after)) in a.values.iter().zip(&b.values).enumerate() {
            if before == after {
                continue;
            }
            use crate::backend::DeviceKind::*;
            let accept = match a.device.kind {
                Joystick => j >= 128 || after == 1,
                Mouse => j != 0,
                Keyboard | XInput => true,
            };
            if !accept {
                continue;
            }
            let button_value = match a.device.kind {
                Joystick if j >= 128 => after,
                Keyboard => 1,
                _ => after,
            };
            return Some(InputDevice {
                bind: InputBinding::Ptt, // the caller sets the real binding
                device_name: a.device.name.replace('\0', ""),
                button: j as i32,
                button_value,
                instance_guid: a.device.instance_guid.clone(),
            });
        }
    }
    None
}
