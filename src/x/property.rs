//! Properties on the server

use crate::{
    core::Window,
    geometry::{Dimension, Point, Ratio, Rectangle},
    tree::Node,
    x::{input::ModMask, xconnection::Atoms},
};
use anyhow::{anyhow, Result};
use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use std::{
    cmp,
    collections::HashMap,
    fmt,
    ops::{Add, Div, Mul, Sub},
};
use x11rb::{
    properties,
    protocol::{xproto, Event},
    rust_connection::Stream,
};

// ============================ SizeHints =============================

bitflags! {
    /// The flags used inside WmSizeHints.
    #[derive(Default)]
    pub struct WmSizeHintsFlags: u32 {
        /// User-specified x and y
        const US_POSITION   = 0b00_0000_0001;
        /// User-specified window size
        const US_SIZE       = 0b00_0000_0010;
        /// Program-specified position
        const P_POSITION    = 0b00_0000_0100;
        /// Program-specified size
        const P_SIZE        = 0b00_0000_1000;
        /// Program-specified minimum size
        const P_MIN_SIZE    = 0b00_0001_0000;
        /// Program specified maximum size
        const P_MAX_SIZE    = 0b00_0010_0000;
        /// Program specified resize increments
        const P_RESIZE_INC  = 0b00_0100_0000;
        /// Program specified aspect ratios
        const P_ASPECT      = 0b00_1000_0000;
        /// Program specified base size
        const P_BASE_SIZE   = 0b01_0000_0000;
        /// Program specified window gravity
        const P_WIN_GRAVITY = 0b10_0000_0000;
    }
}

/// The length of the data for WM_HINTS.
const WM_HINTS_LEN: usize = 9;

/// The length of the data for WM_SIZE_HINTS.
const WM_SIZE_HINTS_LEN: usize = 18;

// #[derive(Debug, Default, Copy, Clone)]
// pub struct WmSizeHints {
//     pub position:       Option<(WmSizeHintsSpecification, i32, i32)>,
//     pub size:           Option<(WmSizeHintsSpecification, i32, i32)>,
//     pub min_size:       Option<(i32, i32)>,
//     pub max_size:       Option<(i32, i32)>,
//     /// `base_size`.
//     pub size_increment: Option<(i32, i32)>,
//     pub aspect:         Option<(Ratio, Ratio)>,
//     pub base_size:      Option<(i32, i32)>,
//     pub win_gravity:    Option<xproto::Gravity>,
// }

/// Structure representing a `WM_SIZE_HINTS` property
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub(crate) struct WmSizeHints {
    /// Flags given to the size hints
    pub(crate) user:           bool,
    /// The position that the window should be assigned
    pub(crate) position:       Option<(i32, i32)>,
    /// The size that the window should be assigned
    pub(crate) size:           Option<(i32, i32)>,
    /// The minimum size that the window may be assigned
    pub(crate) min_size:       Option<(i32, i32)>,
    /// The maximum size that the window may be assigned
    pub(crate) max_size:       Option<(i32, i32)>,
    /// The increment to be used for sizing the window together wit
    pub(crate) size_increment: Option<(i32, i32)>,
    /// The minimum aspect ratio
    pub(crate) min_aspect:     Option<(i32, i32)>,
    /// The maximum aspect ratio
    pub(crate) max_aspect:     Option<(i32, i32)>,
    /// The base size of the window.
    ///
    /// This is used together with `size_increment`.
    pub(crate) base_size:      Option<(i32, i32)>,
    /// The gravity that is used to make room for window decorations.
    pub(crate) gravity:        Option<xproto::Gravity>,
}

impl WmSizeHints {
    /// Create a new [`WmSizeHints`]
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

// - Aspect ratio
// - Gravity
// - Increments

/// Structure representing a `WM_SIZE_HINTS` property
#[derive(Debug, Copy, Clone, PartialOrd, Serialize, Deserialize)]
pub(crate) struct SizeHints {
    /// User flags
    pub(crate) by_user:          bool,
    /// User-specified size
    pub(crate) position:         Option<Point>,
    /// Program-specified base width
    pub(crate) base_width:       Option<u32>,
    /// Program-specified base height
    pub(crate) base_height:      Option<u32>,
    /// Program-specified minimum width
    pub(crate) min_width:        Option<u32>,
    /// Program-specified minimum height
    pub(crate) min_height:       Option<u32>,
    /// Program-specified maximum width
    pub(crate) max_width:        Option<u32>,
    /// Program-specified maximum height
    pub(crate) max_height:       Option<u32>,
    /// Program-specified resize increment for width
    pub(crate) inc_width:        Option<u32>,
    /// Program-specified resize increment for height
    pub(crate) inc_height:       Option<u32>,
    /// Program-specified minimum aspect ratio
    pub(crate) min_ratio:        Option<f64>,
    /// Program-specified maximum aspect ratio
    pub(crate) max_ratio:        Option<f64>,
    pub(crate) min_ratio_vulgar: Option<Ratio>,
    pub(crate) max_ratio_vulgar: Option<Ratio>,
}

impl PartialEq for SizeHints {
    fn eq(&self, other: &Self) -> bool {
        self.min_width == other.min_width
            && self.min_height == other.min_height
            && self.max_width == other.max_width
            && self.max_height == other.max_height
            && self.base_width == other.base_width
            && self.base_height == other.base_height
            && self.inc_width == other.inc_width
            && self.inc_height == other.inc_height
            && self.min_ratio_vulgar == other.min_ratio_vulgar
            && self.max_ratio_vulgar == other.max_ratio_vulgar
    }
}

/// Cannot implement [`Eq`] for [`f64`]
impl Eq for SizeHints {}

// ============================== Hints ===============================

/// TODO: document
#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub(crate) struct Hints {
    pub(crate) urgent:        bool,
    pub(crate) input:         Option<bool>,
    pub(crate) initial_state: Option<IcccmWindowState>,
    pub(crate) group:         Option<Window>,
}

impl Hints {
    /// Create a new [`Hints`]
    pub(crate) const fn new(
        urgent: bool,
        input: Option<bool>,
        initial_state: Option<IcccmWindowState>,
        group: Option<Window>,
    ) -> Self {
        Self {
            urgent,
            input,
            initial_state,
            group,
        }
    }
}

// ======================= Icccm Window State ======================

// NOTE: x11::properties::WmHintsState
/// Possible values for setting the `WM_STATE` property on a client.
///
/// See the [ICCCM docs][1] for more information.
///
/// [1]: https://tronche.com/gui/x/icccm/sec-4.html#s-4.1.3.1
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum IcccmWindowState {
    /// Newly created windows
    Withdrawn,
    /// Window is visible
    Normal,
    /// Window's icon is visible
    Iconic,
}

impl IcccmWindowState {
    /// Convert [`IcccmWindowState`] to a [`WmHintsState`][1]
    ///
    /// [1]: x11::properties::WmHintsState
    pub(crate) const fn to_wmhintsstate(self) -> Option<properties::WmHintsState> {
        match self {
            Self::Normal => Some(properties::WmHintsState::Normal),
            Self::Iconic => Some(properties::WmHintsState::Iconic),
            Self::Withdrawn => None,
        }
    }
}

impl From<properties::WmHintsState> for IcccmWindowState {
    fn from(u: properties::WmHintsState) -> Self {
        match u {
            properties::WmHintsState::Iconic => Self::Iconic,
            properties::WmHintsState::Normal => Self::Normal,
        }
    }
}

impl From<IcccmWindowState> for u32 {
    fn from(u: IcccmWindowState) -> Self {
        match u {
            IcccmWindowState::Withdrawn => 0,
            IcccmWindowState::Normal => 1,
            IcccmWindowState::Iconic => 3,
        }
    }
}

// /// ICCCM-defined window properties.
// pub struct XWinProperties {
//     pub(crate) wm_name: String,
//     pub(crate) wm_icon_name: String,
//     pub(crate) wm_size_hints: Option<WmSizeHints>,
//     pub(crate) wm_hints: Option<WmHints>,
//     pub(crate) wm_class: (String, String), //Instance, Class
//     pub(crate) wm_protocols: Option<Vec<XAtom>>,
//     pub(crate) wm_state: Option<WindowState>,
// }
