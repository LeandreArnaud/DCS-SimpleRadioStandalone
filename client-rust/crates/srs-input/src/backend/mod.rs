//! Device backends.

use srs_settings::InputDevice;

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// DirectInput instance GUID (`Guid.ToString()` form) or the XInput
    /// pseudo-GUID.
    pub instance_guid: String,
    pub product_guid: String,
    pub name: String,
    pub kind: DeviceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Keyboard,
    Mouse,
    Joystick,
    XInput,
}

/// A snapshot of every pressable input of a device, in the C# `AssignButton`
/// layout: 128 buttons then 4 POV values (joystick), 128 keys (keyboard),
/// buttons (mouse), 16 flag values (XInput).
#[derive(Debug, Clone)]
pub struct DeviceSnapshot {
    pub device: DeviceInfo,
    pub values: Vec<i32>,
}

pub trait InputBackend: Send {
    /// Enumerate and acquire devices (`InitDevices`).
    fn rescan(&mut self);
    fn devices(&self) -> Vec<DeviceInfo>;
    /// `GetButtonState`: `None` when the device is unknown.
    fn button_state(&mut self, binding: &InputDevice) -> Option<bool>;
    /// Current values of every device, for button assignment.
    fn snapshot(&mut self) -> Vec<DeviceSnapshot>;
}

/// Backend with no devices (non-Windows builds, tests).
#[derive(Debug, Default)]
pub struct NullBackend;

impl InputBackend for NullBackend {
    fn rescan(&mut self) {}
    fn devices(&self) -> Vec<DeviceInfo> {
        Vec::new()
    }
    fn button_state(&mut self, _binding: &InputDevice) -> Option<bool> {
        None
    }
    fn snapshot(&mut self) -> Vec<DeviceSnapshot> {
        Vec::new()
    }
}

pub mod keys;

#[cfg(windows)]
pub mod directinput;
#[cfg(windows)]
pub mod xinput;

/// The platform default backend.
pub fn default_backend(allow_xinput: bool, expand_controls: bool, hwnd: isize) -> Box<dyn InputBackend> {
    #[cfg(windows)]
    {
        Box::new(directinput::DirectInputBackend::new(
            allow_xinput,
            expand_controls,
            hwnd,
        ))
    }
    #[cfg(not(windows))]
    {
        let _ = (allow_xinput, expand_controls, hwnd);
        Box::new(NullBackend)
    }
}
