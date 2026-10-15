//! `InputBindState` and the precedence resolution of
//! `InputDeviceManager.StartPTTListening`.

use srs_settings::{InputBinding, InputDevice};

/// `InputDevice.IsSameBind`: same button, device and value.
pub fn is_same_bind(a: &InputDevice, b: &InputDevice) -> bool {
    a.button == b.button
        && a.instance_guid.eq_ignore_ascii_case(&b.instance_guid)
        && a.button_value == b.button_value
}

#[derive(Debug, Clone)]
pub struct BindState {
    pub main: InputDevice,
    pub main_state: bool,
    pub modifier: Option<InputDevice>,
    pub modifier_state: bool,
    pub is_active: bool,
}

impl BindState {
    pub fn bind(&self) -> InputBinding {
        self.main.bind
    }
}

/// `GenerateBindStateList`: one entry per non-modifier binding present in
/// the profile, in the C# polling order (`Intercom` .. `AwacsOverlayToggle`).
pub fn bind_states_from_profile(
    bindings: &std::collections::BTreeMap<InputBinding, InputDevice>,
) -> Vec<BindState> {
    InputBinding::polled()
        .filter_map(|b| {
            let main = bindings.get(&b)?.clone();
            let modifier = b.modifier().and_then(|m| bindings.get(&m)).cloned();
            Some(BindState {
                main,
                main_state: false,
                modifier,
                modifier_state: false,
                is_active: false,
            })
        })
        .collect()
}

/// Apply the raw button states and the precedence rule: a bind with a
/// modifier beats an earlier bind without one that shares a button, and an
/// earlier bind with a modifier beats a later plain bind on the same button.
pub fn resolve_bind_states(states: &mut [BindState], mut button_state: impl FnMut(&InputDevice) -> bool) {
    for i in 0..states.len() {
        let main = button_state(&states[i].main);
        states[i].main_state = main;
        if let Some(m) = states[i].modifier.clone() {
            let ms = button_state(&m);
            states[i].modifier_state = ms;
            states[i].is_active = main && ms;
        } else {
            states[i].is_active = main;
        }
        if !states[i].is_active {
            continue;
        }
        for j in 0..i {
            if !states[j].is_active {
                continue;
            }
            let (prev, cur) = {
                let (a, b) = states.split_at_mut(i);
                (&mut a[j], &mut b[0])
            };
            if prev.modifier.is_none() && cur.modifier.is_some() {
                if is_same_bind(&prev.main, &cur.main) {
                    prev.is_active = false;
                    break;
                }
                if cur.modifier.as_ref().is_some_and(|m| is_same_bind(&prev.main, m)) {
                    prev.is_active = false;
                    break;
                }
            } else if prev.modifier.is_some() && cur.modifier.is_none() {
                if is_same_bind(&prev.main, &cur.main) {
                    cur.is_active = false;
                    break;
                }
                if prev.modifier.as_ref().is_some_and(|m| is_same_bind(m, &cur.main)) {
                    cur.is_active = false;
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(bind: InputBinding, button: i32) -> InputDevice {
        InputDevice {
            bind,
            device_name: "js".into(),
            button,
            button_value: 0,
            instance_guid: "g1".into(),
        }
    }

    #[test]
    fn modifier_bind_wins_over_plain_bind_on_same_button() {
        // PTT = button 5 ; Switch1 = button 5 + modifier button 7.
        let mut states = vec![
            BindState {
                main: dev(InputBinding::Ptt, 5),
                main_state: false,
                modifier: None,
                modifier_state: false,
                is_active: false,
            },
            BindState {
                main: dev(InputBinding::Switch1, 5),
                main_state: false,
                modifier: Some(dev(InputBinding::ModifierSwitch1, 7)),
                modifier_state: false,
                is_active: false,
            },
        ];
        // Only button 5 held: PTT active, Switch1 not.
        resolve_bind_states(&mut states, |d| d.button == 5);
        assert!(states[0].is_active);
        assert!(!states[1].is_active);
        // 5 + 7 held: Switch1 wins, PTT suppressed.
        resolve_bind_states(&mut states, |d| d.button == 5 || d.button == 7);
        assert!(!states[0].is_active);
        assert!(states[1].is_active);
    }

    #[test]
    fn earlier_modifier_bind_suppresses_later_plain_bind() {
        let mut states = vec![
            BindState {
                main: dev(InputBinding::Switch1, 5),
                main_state: false,
                modifier: Some(dev(InputBinding::ModifierSwitch1, 7)),
                modifier_state: false,
                is_active: false,
            },
            BindState {
                main: dev(InputBinding::Ptt, 7),
                main_state: false,
                modifier: None,
                modifier_state: false,
                is_active: false,
            },
        ];
        resolve_bind_states(&mut states, |d| d.button == 5 || d.button == 7);
        assert!(states[0].is_active);
        assert!(!states[1].is_active, "PTT on the modifier button is suppressed");
    }
}
