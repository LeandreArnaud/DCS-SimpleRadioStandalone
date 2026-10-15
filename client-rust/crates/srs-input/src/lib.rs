//! Input bindings for PTT and radio control.
//!
//! - [`bind_state`]: pure resolution of main + modifier bindings with the C#
//!   precedence rules (`InputDeviceManager.StartPTTListening`).
//! - [`actions`]: edge detection of the non-PTT actions (`_lastActiveBinding`).
//! - [`backend`]: device access. DirectInput/XInput on Windows; a stub
//!   elsewhere (so the client builds and runs on macOS/Linux without inputs).
//! - [`poller`]: the 40 ms polling thread.

pub mod actions;
pub mod backend;
pub mod bind_state;
pub mod poller;

pub use actions::{ActionTracker, InputAction};
pub use backend::{DeviceInfo, InputBackend};
pub use bind_state::{resolve_bind_states, BindState};
pub use poller::{Poller, PollerHandle};
pub use srs_settings::{InputBinding, InputDevice};
