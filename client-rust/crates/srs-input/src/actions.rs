//! Non-PTT actions with key-repeat suppression (`_lastActiveBinding`).

use srs_settings::InputBinding;

use crate::bind_state::BindState;

/// What the application should do this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    /// `OverlayToggle` (debounced by the UI).
    ToggleOverlay,
    /// `AwacsOverlayToggle`.
    ToggleAwacsOverlay,
    /// One of `Up100`..`RadioVolumeDown` fired (edge).
    Radio(InputBinding),
}

#[derive(Debug)]
pub struct ActionTracker {
    /// `_lastActiveBinding`; `ModifierIntercom` stands for "none".
    last_active: InputBinding,
}

impl Default for ActionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionTracker {
    pub fn new() -> Self {
        Self {
            last_active: InputBinding::ModifierIntercom,
        }
    }

    /// Port of the "handle overlay" loop after the callback: at most one
    /// action per tick, in binding order.
    pub fn actions(&mut self, states: &[BindState]) -> Vec<InputAction> {
        let mut out = Vec::new();
        for s in states {
            let bind = s.bind();
            if s.is_active && bind == InputBinding::OverlayToggle {
                out.push(InputAction::ToggleOverlay);
                break;
            }
            if s.is_active && bind == InputBinding::AwacsOverlayToggle {
                out.push(InputAction::ToggleAwacsOverlay);
                break;
            }
            if (InputBinding::Up100.value()..=InputBinding::AwacsOverlayToggle.value())
                .contains(&bind.value())
            {
                if bind == self.last_active && !s.is_active {
                    self.last_active = InputBinding::ModifierIntercom;
                }
                if s.is_active && bind != self.last_active {
                    self.last_active = bind;
                    out.push(InputAction::Radio(bind));
                    break;
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use srs_settings::InputDevice;

    fn state(bind: InputBinding, active: bool) -> BindState {
        BindState {
            main: InputDevice {
                bind,
                device_name: "k".into(),
                button: 1,
                button_value: 0,
                instance_guid: "g".into(),
            },
            main_state: active,
            modifier: None,
            modifier_state: false,
            is_active: active,
        }
    }

    #[test]
    fn edge_triggered_radio_actions() {
        let mut t = ActionTracker::new();
        let held = vec![state(InputBinding::Up1, true)];
        assert_eq!(t.actions(&held), vec![InputAction::Radio(InputBinding::Up1)]);
        assert!(t.actions(&held).is_empty(), "no repeat while held");
        let released = vec![state(InputBinding::Up1, false)];
        assert!(t.actions(&released).is_empty());
        assert_eq!(
            t.actions(&held),
            vec![InputAction::Radio(InputBinding::Up1)],
            "fires again after release"
        );
    }

    #[test]
    fn overlay_toggle_repeats_every_tick() {
        let mut t = ActionTracker::new();
        let held = vec![state(InputBinding::OverlayToggle, true)];
        assert_eq!(t.actions(&held), vec![InputAction::ToggleOverlay]);
        assert_eq!(t.actions(&held), vec![InputAction::ToggleOverlay]);
    }
}
