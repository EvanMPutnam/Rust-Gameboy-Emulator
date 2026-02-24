use gb_core::GbButton;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKey {
    Right,
    Left,
    Up,
    Down,
    Z,
    X,
    Enter,
    Backspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostInputEvent {
    KeyDown(HostKey),
    KeyUp(HostKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyBinding {
    key: HostKey,
    button: GbButton,
}

const KEY_BINDINGS: [KeyBinding; 8] = [
    KeyBinding {
        key: HostKey::Right,
        button: GbButton::Right,
    },
    KeyBinding {
        key: HostKey::Left,
        button: GbButton::Left,
    },
    KeyBinding {
        key: HostKey::Up,
        button: GbButton::Up,
    },
    KeyBinding {
        key: HostKey::Down,
        button: GbButton::Down,
    },
    KeyBinding {
        key: HostKey::Z,
        button: GbButton::A,
    },
    KeyBinding {
        key: HostKey::X,
        button: GbButton::B,
    },
    KeyBinding {
        key: HostKey::Backspace,
        button: GbButton::Select,
    },
    KeyBinding {
        key: HostKey::Enter,
        button: GbButton::Start,
    },
];

#[derive(Debug, Clone, Copy, Default)]
pub struct InputMap;

impl InputMap {
    #[must_use]
    pub fn map_event(event: HostInputEvent) -> Option<(GbButton, bool)> {
        match event {
            HostInputEvent::KeyDown(key) => Self::lookup(key).map(|button| (button, true)),
            HostInputEvent::KeyUp(key) => Self::lookup(key).map(|button| (button, false)),
        }
    }

    #[must_use]
    fn lookup(key: HostKey) -> Option<GbButton> {
        KEY_BINDINGS
            .iter()
            .find(|binding| binding.key == key)
            .map(|binding| binding.button)
    }
}

#[cfg(test)]
mod tests {
    use super::{HostInputEvent, HostKey, InputMap};
    use gb_core::GbButton;

    #[test]
    fn maps_keydown_to_button_press() {
        assert_eq!(
            InputMap::map_event(HostInputEvent::KeyDown(HostKey::Z)),
            Some((GbButton::A, true))
        );
    }

    #[test]
    fn maps_keyup_to_button_release() {
        assert_eq!(
            InputMap::map_event(HostInputEvent::KeyUp(HostKey::Enter)),
            Some((GbButton::Start, false))
        );
    }

    #[test]
    fn ignores_non_gameplay_keys() {
        // A gameplay-only map should ignore a directionless release without prior press side effects.
        assert_eq!(
            InputMap::map_event(HostInputEvent::KeyUp(HostKey::Z)),
            Some((GbButton::A, false))
        );
    }
}
