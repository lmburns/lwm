//! Input into the window manager

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use x11rb::protocol::xproto::{self, Button as XButton, Keycode as XKeycode, ModMask as XModMask};

// ============================== ModMask =============================
// ====================================================================

// TODO: Add support for all of these
// super hyper meta alt control ctrl shift mode_switch lock mod1 mod2 mod3 mod4 mod5 any

/// Keycode modifier that is held
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
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
