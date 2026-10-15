//! XInput controller (user index 0), masquerading as a device with the fixed
//! GUID the C# client uses (`XInputController.DeviceGuid`). Bindings store
//! the `GamepadButtonFlags` bit value.

use windows::Win32::UI::Input::XboxController::{XInputGetState, XINPUT_STATE};

use super::{DeviceInfo, DeviceKind};

pub const XINPUT_GUID: &str = "bb78c26f-9bfd-41c1-81f0-2f1258d25075";

/// `GamepadButtonFlags` values in enum order (what `AssignButton` iterates).
pub const XINPUT_FLAGS: [u16; 15] = [
    0x0001, 0x0002, 0x0004, 0x0008, 0x0010, 0x0020, 0x0040, 0x0080, 0x0100, 0x0200, 0x1000, 0x2000, 0x4000,
    0x8000, 0x0000,
];

pub struct XInputDevice {
    info: DeviceInfo,
}

impl Default for XInputDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl XInputDevice {
    pub fn new() -> Self {
        Self {
            info: DeviceInfo {
                instance_guid: XINPUT_GUID.into(),
                product_guid: XINPUT_GUID.into(),
                name: "XInputController".into(),
                kind: DeviceKind::XInput,
            },
        }
    }

    pub fn info(&self) -> DeviceInfo {
        self.info.clone()
    }

    fn buttons(&self) -> u16 {
        let mut state = XINPUT_STATE::default();
        let r = unsafe { XInputGetState(0, &mut state) };
        if r == 0 {
            state.Gamepad.wButtons.0
        } else {
            0
        }
    }

    pub fn has_flag(&mut self, flag: i32) -> bool {
        let f = u16::try_from(flag).unwrap_or(0);
        f != 0 && self.buttons() & f == f
    }

    /// One value per `GamepadButtonFlags` member, like the C# snapshot.
    pub fn snapshot(&mut self) -> Vec<i32> {
        let b = self.buttons();
        XINPUT_FLAGS
            .iter()
            .map(|&f| i32::from(f != 0 && b & f == f))
            .collect()
    }
}
