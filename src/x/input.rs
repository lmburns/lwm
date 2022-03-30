//! Input into the window manager

use crate::{core::Window, geometry::Point};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use strum::{EnumIter, IntoEnumIterator};
use x11rb::protocol::xproto::{
    self,
    Button as XButton,
    ButtonPressEvent,
    ButtonReleaseEvent,
    KeyPressEvent,
    Keycode as XKeycode,
    ModMask as XModMask,
    MotionNotifyEvent,
};

// ============================== ModMask =============================
// ====================================================================

// TODO: Add support for all of these
// super hyper meta alt control ctrl shift mode_switch lock mod1 mod2 mod3 mod4
// mod5 any

/// Keycode modifier that is held
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize, EnumIter,
)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ModMask {
    /// Left or right `shift` key
    Shift,
    /// Num-lock, scroll-lock TODO: (maybe caps-lock?)
    Lock,
    /// Left or right `control` key
    #[serde(alias = "ctrl")]
    Control,
    /// Modifier 1 as defined in `xmodmap` (usually `alt`)
    Mod1,
    /// Modifier 2 as defined in `xmodmap` (usually `num-lock`)
    Mod2,
    /// Modifier 3 as defined in `xmodmap` (usually blank)
    Mod3,
    /// Modifier 4 as defined in `xmodmap` (usually `super`)
    Mod4,
    /// Modifier 5 as defined or in `xmodmap` (usually `mode_shift`)
    Mod5,
    /// Catch all, used with the X11 interface
    #[serde(skip_deserializing)]
    Any,
}

impl ModMask {
    pub(crate) fn was_held(&self, mask: u16) -> bool {
        mask & u16::from(*self) > 0
    }
}

/// Convert from an [`x11rb`] [`ModMask`](xproto::ModMask) to a [`ModMask`]
impl From<ModMask> for XModMask {
    fn from(m: ModMask) -> Self {
        match m {
            ModMask::Shift => Self::SHIFT,
            ModMask::Lock => Self::LOCK,
            ModMask::Control => Self::CONTROL,
            ModMask::Mod1 => Self::M1,
            ModMask::Mod2 => Self::M2,
            ModMask::Mod3 => Self::M3,
            ModMask::Mod4 => Self::M4,
            ModMask::Mod5 => Self::M5,
            ModMask::Any => Self::ANY,
        }
    }
}

impl From<ModMask> for u16 {
    fn from(m: ModMask) -> Self {
        u16::from(match m {
            ModMask::Shift => XModMask::SHIFT,
            ModMask::Lock => XModMask::LOCK,
            ModMask::Control => XModMask::CONTROL,
            ModMask::Mod1 => XModMask::M1,
            ModMask::Mod2 => XModMask::M2,
            ModMask::Mod3 => XModMask::M3,
            ModMask::Mod4 => XModMask::M4,
            ModMask::Mod5 => XModMask::M5,
            ModMask::Any => XModMask::ANY,
        })
    }
}

impl fmt::Display for ModMask {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

// impl From<u16> for ModMask {
//     fn from(mask: u16) -> ModifierMask {
//         ModifierMask::new(mask)
//     }
// }

// ============================== Keycode =============================
// ====================================================================

/// Key press and its [`ModMask`]
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct Keycode {
    /// Held modifier mask
    pub(crate) mask: ModMask,
    /// Keycode that was held
    pub(crate) code: XKeycode,
}

// impl KeyCode {
//     /// Create a new [KeyCode] from this one that removes the given mask
//     pub fn ignoring_modifier(&self, mask: ModMask) -> Self {
//         Self {
//             mask: self.mask & !mask,
//             code: self.code,
//         }
//     }
// }

// ============================== Button ==============================
// ====================================================================

/// Available buttons on a mouse
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Button {
    /// 1, Left-click
    #[serde(rename = "mouse1", alias = "button1")]
    Left,
    /// 2, Middle-click
    #[serde(rename = "mouse2", alias = "button2")]
    Middle,
    /// 3, Right-click
    #[serde(rename = "mouse3", alias = "button3")]
    Right,
    /// 4, Wheel-scroll up
    #[serde(alias = "scroll-up", alias = "scroll_up")]
    ScrollUp,
    /// 5, Wheel-scroll down
    #[serde(alias = "scroll-down", alias = "scroll_down")]
    ScrollDown,
}

impl From<Button> for XButton {
    fn from(b: Button) -> Self {
        match b {
            Button::Left => 1,
            Button::Middle => 2,
            Button::Right => 3,
            Button::ScrollUp => 4,
            Button::ScrollDown => 5,
        }
    }
}

impl TryFrom<u8> for Button {
    type Error = anyhow::Error;

    fn try_from(u: u8) -> Result<Self> {
        match u {
            1 => Ok(Self::Left),
            2 => Ok(Self::Middle),
            3 => Ok(Self::Right),
            4 => Ok(Self::ScrollUp),
            5 => Ok(Self::ScrollDown),
            _ => Err(anyhow!("mouse button {} is unknown", u)),
        }
    }
}

// ============================ MouseState ============================
// ====================================================================

// Mouse state specification, which indicates the [`Button`] and modifiers held
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub(crate) struct MouseState {
    /// The [`Button`] being held
    pub(crate) button:  Button,
    /// The [`ModMask`] of all held modifiers
    pub(crate) modmask: Vec<ModMask>,
}

impl MouseState {
    /// Create a new [`MouseState`]
    pub(crate) fn new(button: Button, mut modmask: Vec<ModMask>) -> Self {
        modmask.sort();
        Self { button, modmask }
    }

    pub(crate) fn from_event(detail: u8, state: u16) -> Result<Self> {
        Ok(Self {
            button:  Button::try_from(detail)?,
            modmask: ModMask::iter().filter(|m| m.was_held(state)).collect(),
        })
    }

    pub(crate) fn mask(&self) -> u16 {
        self.modmask
            .iter()
            .fold(0, |acc, &val| acc | u16::from(val))
    }

    pub(crate) fn button(&self) -> u8 {
        self.button.into()
    }
}

// ============================ MouseEvent ============================
// ====================================================================

/// The types of mouse events represented by a `MouseEvent`
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(crate) enum MouseEventKind {
    /// Button was pressed
    Press,
    /// Button was released
    Release,
    /// Mouse is moved while a [`Button is held`]
    Motion,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(crate) enum EventTarget {
    Global,
    Root,
    Client,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(crate) struct MouseEventKey {
    pub(crate) kind:   MouseEventKind,
    pub(crate) target: EventTarget,
}

#[derive(Debug, Clone)]
pub(crate) struct MouseEvent {
    pub(crate) kind:        MouseEventKind,
    pub(crate) window:      Window,
    pub(crate) subwindow:   Option<Window>,
    pub(crate) on_root:     bool,
    pub(crate) root_rpos:   Point,
    pub(crate) window_rpos: Point,
    pub(crate) shortcut:    MouseState,
}

impl MouseEvent {
    pub fn new(
        kind: MouseEventKind,
        window: Window,
        subwindow: Option<Window>,
        root: Window,
        root_rx: i16,
        root_ry: i16,
        window_rx: i16,
        window_ry: i16,
        shortcut: MouseState,
    ) -> Self {
        Self {
            kind,
            window,
            subwindow,
            on_root: window == root,
            root_rpos: Point {
                x: root_rx as i32,
                y: root_ry as i32,
            },
            window_rpos: Point {
                x: window_rx as i32,
                y: window_ry as i32,
            },
            shortcut,
        }
    }

    pub(crate) fn from_press_event(event: &ButtonPressEvent, root: Window) -> Result<Self> {
        Ok(Self::new(
            MouseEventKind::Press,
            event.event,
            if event.child != x11rb::NONE {
                Some(event.child)
            } else {
                None
            },
            root,
            event.root_x,
            event.root_y,
            event.event_x,
            event.event_y,
            MouseState::from_event(event.detail, event.state)?,
        ))
    }

    pub(crate) fn from_release_event(event: &ButtonReleaseEvent, root: Window) -> Result<Self> {
        Ok(Self::new(
            MouseEventKind::Release,
            event.event,
            if event.child != x11rb::NONE {
                Some(event.child)
            } else {
                None
            },
            root,
            event.root_x,
            event.root_y,
            event.event_x,
            event.event_y,
            MouseState::from_event(event.detail, event.state)?,
        ))
    }

    pub(crate) fn from_motion_event(event: &MotionNotifyEvent, root: Window) -> Result<Self> {
        Ok(Self::new(
            MouseEventKind::Motion,
            event.event,
            if event.child != x11rb::NONE {
                Some(event.child)
            } else {
                None
            },
            root,
            event.root_x,
            event.root_y,
            event.event_x,
            event.event_y,
            MouseState::from_event(1, event.state)?,
        ))
    }
}
