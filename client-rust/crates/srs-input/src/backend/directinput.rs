//! DirectInput 8 backend (Windows): keyboard, mouse and joysticks with the
//! same device identity (instance GUID) and button indexes as SharpDX, so
//! profiles written by the C# client keep working.
//!
//! Keyboard bindings are indexes into `SharpDX.DirectInput.Key` (see
//! `keys.rs`), joystick bindings are `DIJOYSTATE2.rgbButtons` indexes (POV
//! hats are `128 + n` with the angle as value), mouse bindings are
//! `DIMOUSESTATE2.rgbButtons` indexes.

use std::collections::HashSet;

use srs_settings::InputDevice;
use windows::core::{Interface, BOOL, GUID};
use windows::Win32::Devices::HumanInterfaceDevice::*;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use super::keys::SHARPDX_KEY_DIK;
use super::xinput::XInputDevice;
use super::{DeviceInfo, DeviceKind, DeviceSnapshot, InputBackend};

// Standard data formats exported by dinput8.dll.
#[link(name = "dinput8", kind = "static")]
#[link(name = "dxguid", kind = "static")]
extern "C" {
    static c_dfDIJoystick2: DIDATAFORMAT;
    static c_dfDIKeyboard: DIDATAFORMAT;
    static c_dfDIMouse2: DIDATAFORMAT;
}

const DISCL_FLAGS: u32 = DISCL_BACKGROUND | DISCL_NONEXCLUSIVE;
const DI8DEVTYPE_MOUSE_: u32 = 18;
const DI8DEVTYPE_KEYBOARD_: u32 = 19;
const DI8DEVTYPE_JOYSTICK_: u32 = 20;
const DI8DEVTYPE_FIRSTPERSON_: u32 = 24;

/// Devices the C# client ignores (keyboards/mice/audio reporting as joysticks).
const BLACKLIST: &[&str] = &[
    "1b171b1c-0000-0000-0000-504944564944",
    "1b091b1c-0000-0000-0000-504944564944",
    "1b1e1b1c-0000-0000-0000-504944564944",
    "16a40951-0000-0000-0000-504944564944",
    "b660044f-0000-0000-0000-504944564944",
    "00f2068e-0000-0000-0000-504944564944",
];

/// Devices with an odd type that must still be treated as joysticks.
const WHITELIST: &[&str] = &[
    "1105231d-0000-0000-0000-504944564944",
    "b351044f-0000-0000-0000-504944564944",
    "b352044f-0000-0000-0000-504944564944",
    "b353044f-0000-0000-0000-504944564944",
    "b354044f-0000-0000-0000-504944564944",
    "b355044f-0000-0000-0000-504944564944",
    "b356044f-0000-0000-0000-504944564944",
    "b357044f-0000-0000-0000-504944564944",
    "b358044f-0000-0000-0000-504944564944",
    "11401dd2-0000-0000-0000-504944564944",
    "204803eb-0000-0000-0000-504944564944",
    "204303eb-0000-0000-0000-504944564944",
    "205403eb-0000-0000-0000-504944564944",
    "205603eb-0000-0000-0000-504944564944",
    "205503eb-0000-0000-0000-504944564944",
    "82c43344-0000-0000-0000-504944564944",
    "c2ab046d-0000-0000-0000-504944564944",
    "aaaa3344-0000-0000-0000-504944564944",
];

/// `Guid.ToString()` ("D" format, lowercase).
pub fn guid_to_string(g: &GUID) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        g.data1,
        g.data2,
        g.data3,
        g.data4[0],
        g.data4[1],
        g.data4[2],
        g.data4[3],
        g.data4[4],
        g.data4[5],
        g.data4[6],
        g.data4[7]
    )
}

fn wide_to_string(w: &[u16]) -> String {
    let end = w.iter().position(|&c| c == 0).unwrap_or(w.len());
    String::from_utf16_lossy(&w[..end]).trim().replace('\0', "")
}

struct DiDevice {
    info: DeviceInfo,
    guid: GUID,
    dev: Option<IDirectInputDevice8W>,
}

pub struct DirectInputBackend {
    di: Option<IDirectInput8W>,
    devices: Vec<DiDevice>,
    xinput: Option<XInputDevice>,
    allow_xinput: bool,
    expand_controls: bool,
    hwnd: HWND,
    whitelist: HashSet<String>,
    blacklist: HashSet<String>,
}

// The COM pointers are only used from the polling thread.
unsafe impl Send for DirectInputBackend {}

impl DirectInputBackend {
    pub fn new(allow_xinput: bool, expand_controls: bool, hwnd: isize) -> Self {
        let mut whitelist: HashSet<String> = WHITELIST.iter().map(|s| s.to_string()).collect();
        let mut blacklist: HashSet<String> = BLACKLIST.iter().map(|s| s.to_string()).collect();
        load_guid_list("whitelist.txt", &mut whitelist);
        load_guid_list("blacklist.txt", &mut blacklist);
        let di = unsafe {
            let hinst = GetModuleHandleW(None).ok();
            let mut ptr: *mut core::ffi::c_void = core::ptr::null_mut();
            match DirectInput8Create(
                hinst.unwrap_or_default().into(),
                DIRECTINPUT_VERSION,
                &IDirectInput8W::IID,
                &mut ptr,
                None,
            ) {
                Ok(()) if !ptr.is_null() => Some(IDirectInput8W::from_raw(ptr)),
                Ok(()) => None,
                Err(e) => {
                    log::error!("DirectInput8Create failed: {e}");
                    None
                }
            }
        };
        let mut b = Self {
            di,
            devices: Vec::new(),
            xinput: None,
            allow_xinput,
            expand_controls,
            hwnd: HWND(hwnd as *mut core::ffi::c_void),
            whitelist,
            blacklist,
        };
        b.rescan();
        b
    }

    fn open(&self, inst: &DIDEVICEINSTANCEW, kind: DeviceKind) -> Option<IDirectInputDevice8W> {
        let di = self.di.as_ref()?;
        unsafe {
            let mut dev: Option<IDirectInputDevice8W> = None;
            if let Err(e) = di.CreateDevice(&inst.guidInstance, &mut dev, None) {
                log::warn!(
                    "CreateDevice failed for {}: {e}",
                    wide_to_string(&inst.tszProductName)
                );
                return None;
            }
            let dev = dev?;
            let fmt: *const DIDATAFORMAT = match kind {
                DeviceKind::Keyboard => &raw const c_dfDIKeyboard,
                DeviceKind::Mouse => &raw const c_dfDIMouse2,
                _ => &raw const c_dfDIJoystick2,
            };
            if let Err(e) = dev.SetDataFormat(fmt as *mut DIDATAFORMAT) {
                log::warn!("SetDataFormat failed: {e}");
                return None;
            }
            if let Err(e) = dev.SetCooperativeLevel(self.hwnd, DISCL_FLAGS) {
                log::warn!("SetCooperativeLevel failed: {e}");
                return None;
            }
            if let Err(e) = dev.Acquire() {
                log::warn!("Acquire failed (will retry on poll): {e}");
            }
            Some(dev)
        }
    }

    fn find(&mut self, instance_guid: &str) -> Option<&mut DiDevice> {
        self.devices
            .iter_mut()
            .find(|d| d.info.instance_guid.eq_ignore_ascii_case(instance_guid))
    }
}

fn load_guid_list(file: &str, set: &mut HashSet<String>) {
    if let Ok(text) = std::fs::read_to_string(file) {
        for line in text.lines() {
            let t = line
                .trim()
                .trim_matches(|c| c == '{' || c == '}')
                .to_ascii_lowercase();
            if t.len() == 36 {
                set.insert(t);
            }
        }
    }
}

unsafe extern "system" fn enum_cb(inst: *mut DIDEVICEINSTANCEW, ctx: *mut core::ffi::c_void) -> BOOL {
    let list = &mut *(ctx as *mut Vec<DIDEVICEINSTANCEW>);
    list.push(*inst);
    BOOL(1) // DIENUM_CONTINUE
}

/// Read a device state with one re-acquire attempt on input loss.
unsafe fn get_state<T: Default>(dev: &IDirectInputDevice8W) -> Option<T> {
    let mut state = T::default();
    let size = core::mem::size_of::<T>() as u32;
    let _ = dev.Poll();
    match dev.GetDeviceState(size, &mut state as *mut T as *mut core::ffi::c_void) {
        Ok(()) => Some(state),
        Err(e) if e.code() == DIERR_INPUTLOST || e.code() == DIERR_NOTACQUIRED => {
            let _ = dev.Acquire();
            let _ = dev.Poll();
            dev.GetDeviceState(size, &mut state as *mut T as *mut core::ffi::c_void)
                .ok()
                .map(|_| state)
        }
        Err(_) => None,
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct KeyboardState([u8; 256]);

impl Default for KeyboardState {
    fn default() -> Self {
        Self([0; 256])
    }
}

impl InputBackend for DirectInputBackend {
    fn rescan(&mut self) {
        self.devices.clear();
        self.xinput = if self.allow_xinput {
            Some(XInputDevice::new())
        } else {
            None
        };
        let Some(di) = self.di.clone() else { return };
        let mut found: Vec<DIDEVICEINSTANCEW> = Vec::new();
        unsafe {
            if let Err(e) = di.EnumDevices(
                DI8DEVCLASS_ALL,
                Some(enum_cb),
                &mut found as *mut _ as *mut core::ffi::c_void,
                DIEDFL_ATTACHEDONLY,
            ) {
                log::error!("EnumDevices failed: {e}");
                return;
            }
        }
        log::info!(
            "Starting Device Search. Expand Search: {}. Use XInput: {}",
            self.expand_controls,
            self.allow_xinput
        );
        for inst in found {
            let product = guid_to_string(&inst.guidProduct);
            let instance = guid_to_string(&inst.guidInstance);
            let name = wide_to_string(&inst.tszProductName);
            let dev_type = inst.dwDevType & 0xFF;
            if self.blacklist.contains(&product) {
                log::info!("Found but ignoring blacklist device {product} Instance: {instance} {name} Type: {dev_type}");
                continue;
            }
            if self.devices.iter().any(|d| d.info.instance_guid == instance) {
                continue;
            }
            let kind = if dev_type == DI8DEVTYPE_KEYBOARD_ {
                DeviceKind::Keyboard
            } else if dev_type == DI8DEVTYPE_MOUSE_ {
                DeviceKind::Mouse
            } else if (DI8DEVTYPE_JOYSTICK_..=DI8DEVTYPE_FIRSTPERSON_).contains(&dev_type)
                || self.whitelist.contains(&product)
            {
                DeviceKind::Joystick
            } else if self.expand_controls {
                DeviceKind::Joystick
            } else {
                log::info!("Skipping device {product} {name} Type: {dev_type} (not a joystick; enable ExpandControls)");
                continue;
            };
            log::info!("Adding Device ID:{product} {name} Type: {dev_type} as {kind:?}");
            let dev = self.open(&inst, kind);
            self.devices.push(DiDevice {
                info: DeviceInfo {
                    instance_guid: instance,
                    product_guid: product,
                    name,
                    kind,
                },
                guid: inst.guidInstance,
                dev,
            });
        }
    }

    fn devices(&self) -> Vec<DeviceInfo> {
        let mut v: Vec<DeviceInfo> = self.devices.iter().map(|d| d.info.clone()).collect();
        if let Some(x) = &self.xinput {
            v.push(x.info());
        }
        v
    }

    fn button_state(&mut self, binding: &InputDevice) -> Option<bool> {
        if let Some(x) = &mut self.xinput {
            if x.info()
                .instance_guid
                .eq_ignore_ascii_case(&binding.instance_guid)
            {
                return Some(x.has_flag(binding.button));
            }
        }
        let d = self.find(&binding.instance_guid)?;
        let dev = d.dev.as_ref()?;
        let _ = d.guid;
        unsafe {
            match d.info.kind {
                DeviceKind::Joystick => {
                    let st: DIJOYSTATE2 = get_state(dev)?;
                    let b = binding.button;
                    if b >= 128 {
                        let pov = usize::try_from(b - 128).ok()?;
                        let v = *st.rgdwPOV.get(pov)? as i32;
                        Some(v == binding.button_value)
                    } else {
                        let i = usize::try_from(b).ok()?;
                        Some(st.rgbButtons.get(i).is_some_and(|v| v & 0x80 != 0))
                    }
                }
                DeviceKind::Keyboard => {
                    let st: KeyboardState = get_state(dev)?;
                    let i = usize::try_from(binding.button).ok()?;
                    let dik = *SHARPDX_KEY_DIK.get(i)? as usize;
                    Some(st.0[dik] & 0x80 != 0)
                }
                DeviceKind::Mouse => {
                    let st: DIMOUSESTATE2 = get_state(dev)?;
                    let i = usize::try_from(binding.button).ok()?;
                    Some(st.rgbButtons.get(i).is_some_and(|v| v & 0x80 != 0))
                }
                DeviceKind::XInput => None,
            }
        }
    }

    fn snapshot(&mut self) -> Vec<DeviceSnapshot> {
        let mut out = Vec::new();
        if let Some(x) = &mut self.xinput {
            out.push(DeviceSnapshot {
                device: x.info(),
                values: x.snapshot(),
            });
        }
        for d in &mut self.devices {
            let Some(dev) = d.dev.as_ref() else { continue };
            let values = unsafe {
                match d.info.kind {
                    DeviceKind::Joystick => {
                        let Some(st): Option<DIJOYSTATE2> = get_state(dev) else {
                            continue;
                        };
                        let mut v: Vec<i32> =
                            st.rgbButtons.iter().map(|b| i32::from(b & 0x80 != 0)).collect();
                        v.extend(st.rgdwPOV.iter().map(|&p| p as i32));
                        v
                    }
                    DeviceKind::Keyboard => {
                        let Some(st): Option<KeyboardState> = get_state(dev) else {
                            continue;
                        };
                        SHARPDX_KEY_DIK
                            .iter()
                            .take(128)
                            .map(|&dik| i32::from(st.0[dik as usize] & 0x80 != 0))
                            .collect()
                    }
                    DeviceKind::Mouse => {
                        let Some(st): Option<DIMOUSESTATE2> = get_state(dev) else {
                            continue;
                        };
                        st.rgbButtons.iter().map(|b| i32::from(b & 0x80 != 0)).collect()
                    }
                    DeviceKind::XInput => continue,
                }
            };
            out.push(DeviceSnapshot {
                device: d.info.clone(),
                values,
            });
        }
        out
    }
}

impl Drop for DirectInputBackend {
    fn drop(&mut self) {
        for d in &self.devices {
            if let Some(dev) = &d.dev {
                unsafe {
                    let _ = dev.Unacquire();
                }
            }
        }
    }
}
