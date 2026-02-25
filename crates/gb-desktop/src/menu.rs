#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmulationSpeed {
    X2,
    X4,
    X8,
    X16,
}

impl EmulationSpeed {
    #[must_use]
    pub const fn multiplier(self) -> u32 {
        match self {
            Self::X2 => 2,
            Self::X4 => 4,
            Self::X8 => 8,
            Self::X16 => 16,
        }
    }

    #[must_use]
    pub const fn from_multiplier(multiplier: u32) -> Option<Self> {
        match multiplier {
            2 => Some(Self::X2),
            4 => Some(Self::X4),
            8 => Some(Self::X8),
            16 => Some(Self::X16),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCommand {
    SaveState,
    LoadState,
    SetEmulationSpeed(EmulationSpeed),
    ToggleMute,
}

#[cfg(target_os = "macos")]
mod imp {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Sel};
    use objc2::{ClassType, DeclaredClass, declare_class, msg_send_id, mutability, sel};
    use objc2_app_kit::{
        NSApplication, NSControlStateValueOff, NSControlStateValueOn, NSEventModifierFlags, NSMenu,
        NSMenuItem,
    };
    use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSString, ns_string};

    use super::{EmulationSpeed, MenuCommand};

    thread_local! {
        static PENDING_COMMANDS: RefCell<VecDeque<MenuCommand>> = const { RefCell::new(VecDeque::new()) };
    }

    fn enqueue_command(command: MenuCommand) {
        PENDING_COMMANDS.with(|pending| pending.borrow_mut().push_back(command));
    }

    fn take_pending_commands() -> Vec<MenuCommand> {
        PENDING_COMMANDS.with(|pending| pending.borrow_mut().drain(..).collect())
    }

    declare_class!(
        struct MenuActionTarget;

        unsafe impl ClassType for MenuActionTarget {
            type Super = NSObject;
            type Mutability = mutability::MainThreadOnly;
            const NAME: &'static str = "GbDesktopMenuActionTarget";
        }

        impl DeclaredClass for MenuActionTarget {}

        unsafe impl NSObjectProtocol for MenuActionTarget {}

        unsafe impl MenuActionTarget {
            #[method(saveState:)]
            fn save_state(&self, _sender: &AnyObject) {
                enqueue_command(MenuCommand::SaveState);
            }

            #[method(loadState:)]
            fn load_state(&self, _sender: &AnyObject) {
                enqueue_command(MenuCommand::LoadState);
            }

            #[method(setSpeed2x:)]
            fn set_speed_2x(&self, _sender: &AnyObject) {
                enqueue_command(MenuCommand::SetEmulationSpeed(EmulationSpeed::X2));
            }

            #[method(setSpeed4x:)]
            fn set_speed_4x(&self, _sender: &AnyObject) {
                enqueue_command(MenuCommand::SetEmulationSpeed(EmulationSpeed::X4));
            }

            #[method(setSpeed8x:)]
            fn set_speed_8x(&self, _sender: &AnyObject) {
                enqueue_command(MenuCommand::SetEmulationSpeed(EmulationSpeed::X8));
            }

            #[method(setSpeed16x:)]
            fn set_speed_16x(&self, _sender: &AnyObject) {
                enqueue_command(MenuCommand::SetEmulationSpeed(EmulationSpeed::X16));
            }

            #[method(toggleMute:)]
            fn toggle_mute(&self, _sender: &AnyObject) {
                enqueue_command(MenuCommand::ToggleMute);
            }
        }
    );

    impl MenuActionTarget {
        fn new(mtm: MainThreadMarker) -> Retained<Self> {
            // SAFETY: `MenuActionTarget` is a valid Objective-C class deriving from `NSObject`,
            // and initializing with `init` is the designated construction path.
            unsafe { msg_send_id![super(mtm.alloc().set_ivars(())), init] }
        }
    }

    fn action_item(
        mtm: MainThreadMarker,
        title: &NSString,
        selector: Sel,
        target: &AnyObject,
        key: Option<&NSString>,
        modifiers: Option<NSEventModifierFlags>,
    ) -> Retained<NSMenuItem> {
        let key_equivalent = key.unwrap_or(ns_string!(""));
        // SAFETY: The selector and key-equivalent are valid Objective-C values for `NSMenuItem`
        // initialization; Cocoa retains the created menu item and the returned object is owned.
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                title,
                Some(selector),
                key_equivalent,
            )
        };
        // SAFETY: The target object remains retained for the menu lifetime in `DesktopMenuInner`,
        // and is a valid Objective-C receiver for the selector attached to this item.
        unsafe { item.setTarget(Some(target)) };
        if let Some(flags) = modifiers {
            item.setKeyEquivalentModifierMask(flags);
        }
        item
    }

    fn set_title(item: &NSMenuItem, title: &NSString) {
        // SAFETY: `title` is a valid `NSString` and `item` is a live menu item on the main thread.
        unsafe { item.setTitle(title) };
    }

    fn set_checked(item: &NSMenuItem, checked: bool) {
        let state = if checked {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        };
        // SAFETY: `state` is one of Cocoa's valid `NSControlStateValue` values.
        unsafe { item.setState(state) };
    }

    struct DesktopMenuInner {
        _action_target: Retained<MenuActionTarget>,
        _menubar: Retained<NSMenu>,
        speed_2x: Retained<NSMenuItem>,
        speed_4x: Retained<NSMenuItem>,
        speed_8x: Retained<NSMenuItem>,
        speed_16x: Retained<NSMenuItem>,
        mute_toggle: Retained<NSMenuItem>,
    }

    pub struct DesktopMenu {
        inner: Option<DesktopMenuInner>,
    }

    impl DesktopMenu {
        #[must_use]
        pub fn install(initial_speed: u32, muted: bool) -> Self {
            let Some(mtm) = MainThreadMarker::new() else {
                return Self { inner: None };
            };

            let app = NSApplication::sharedApplication(mtm);
            let action_target = MenuActionTarget::new(mtm);
            let action_target_ref: &AnyObject = action_target.as_ref();

            let menubar = NSMenu::new(mtm);
            let file_menu_root = NSMenuItem::new(mtm);
            set_title(&file_menu_root, ns_string!("File"));
            menubar.addItem(&file_menu_root);

            let file_menu = NSMenu::new(mtm);
            file_menu_root.setSubmenu(Some(&file_menu));

            let command_modifier = Some(NSEventModifierFlags::NSEventModifierFlagCommand);
            let save_item = action_item(
                mtm,
                ns_string!("Save State"),
                sel!(saveState:),
                action_target_ref,
                Some(ns_string!("s")),
                command_modifier,
            );
            let load_item = action_item(
                mtm,
                ns_string!("Load State"),
                sel!(loadState:),
                action_target_ref,
                Some(ns_string!("l")),
                command_modifier,
            );

            let speed_menu_root = NSMenuItem::new(mtm);
            set_title(&speed_menu_root, ns_string!("Emulation Speed"));
            let speed_menu = NSMenu::new(mtm);
            speed_menu_root.setSubmenu(Some(&speed_menu));

            let speed_2x = action_item(
                mtm,
                ns_string!("2x"),
                sel!(setSpeed2x:),
                action_target_ref,
                None,
                None,
            );
            let speed_4x = action_item(
                mtm,
                ns_string!("4x"),
                sel!(setSpeed4x:),
                action_target_ref,
                None,
                None,
            );
            let speed_8x = action_item(
                mtm,
                ns_string!("8x"),
                sel!(setSpeed8x:),
                action_target_ref,
                None,
                None,
            );
            let speed_16x = action_item(
                mtm,
                ns_string!("16x"),
                sel!(setSpeed16x:),
                action_target_ref,
                None,
                None,
            );

            speed_menu.addItem(&speed_2x);
            speed_menu.addItem(&speed_4x);
            speed_menu.addItem(&speed_8x);
            speed_menu.addItem(&speed_16x);

            let mute_toggle = action_item(
                mtm,
                ns_string!("Mute"),
                sel!(toggleMute:),
                action_target_ref,
                Some(ns_string!("m")),
                command_modifier,
            );

            file_menu.addItem(&save_item);
            file_menu.addItem(&load_item);
            file_menu.addItem(&NSMenuItem::separatorItem(mtm));
            file_menu.addItem(&speed_menu_root);
            file_menu.addItem(&NSMenuItem::separatorItem(mtm));
            file_menu.addItem(&mute_toggle);

            app.setMainMenu(Some(&menubar));

            let menu = Self {
                inner: Some(DesktopMenuInner {
                    _action_target: action_target,
                    _menubar: menubar,
                    speed_2x,
                    speed_4x,
                    speed_8x,
                    speed_16x,
                    mute_toggle,
                }),
            };
            menu.set_emulation_speed(initial_speed);
            menu.set_muted(muted);
            menu
        }

        #[must_use]
        pub fn take_pending_commands() -> Vec<MenuCommand> {
            take_pending_commands()
        }

        pub fn set_emulation_speed(&self, multiplier: u32) {
            let Some(inner) = &self.inner else {
                return;
            };

            let selected = EmulationSpeed::from_multiplier(multiplier);
            set_checked(&inner.speed_2x, selected == Some(EmulationSpeed::X2));
            set_checked(&inner.speed_4x, selected == Some(EmulationSpeed::X4));
            set_checked(&inner.speed_8x, selected == Some(EmulationSpeed::X8));
            set_checked(&inner.speed_16x, selected == Some(EmulationSpeed::X16));
        }

        pub fn set_muted(&self, muted: bool) {
            let Some(inner) = &self.inner else {
                return;
            };
            if muted {
                set_title(&inner.mute_toggle, ns_string!("Unmute"));
            } else {
                set_title(&inner.mute_toggle, ns_string!("Mute"));
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::MenuCommand;

    pub struct DesktopMenu;

    impl DesktopMenu {
        #[must_use]
        pub fn install(_initial_speed: u32, _muted: bool) -> Self {
            Self
        }

        #[must_use]
        pub fn take_pending_commands() -> Vec<MenuCommand> {
            Vec::new()
        }

        pub fn set_emulation_speed(&self, _multiplier: u32) {}

        pub fn set_muted(&self, _muted: bool) {}
    }
}

pub use imp::DesktopMenu;

#[cfg(test)]
mod tests {
    use super::EmulationSpeed;

    #[test]
    fn speed_multiplier_round_trip() {
        assert_eq!(EmulationSpeed::from_multiplier(2), Some(EmulationSpeed::X2));
        assert_eq!(EmulationSpeed::from_multiplier(4), Some(EmulationSpeed::X4));
        assert_eq!(EmulationSpeed::from_multiplier(8), Some(EmulationSpeed::X8));
        assert_eq!(
            EmulationSpeed::from_multiplier(16),
            Some(EmulationSpeed::X16)
        );
        assert_eq!(EmulationSpeed::from_multiplier(1), None);

        assert_eq!(EmulationSpeed::X2.multiplier(), 2);
        assert_eq!(EmulationSpeed::X4.multiplier(), 4);
        assert_eq!(EmulationSpeed::X8.multiplier(), 8);
        assert_eq!(EmulationSpeed::X16.multiplier(), 16);
    }
}
