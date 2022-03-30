//! Interacting with the [`Window`] tree

use crate::{
    core::{Direction, LayoutType, Output, Window, Xid},
    geometry::{Padding, Rectangle},
    monitor::client::Client,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// The type of [`Window`] split
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum SplitType {
    /// Window is split with the axis of the split going from East to West
    Horizontal,
    /// Window is split with the axis of the split going from North to South
    Vertical,
}

/// Mode of splitting a [`Node`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SplitMode {
    /// Splitting is automatically done
    Automatic,
    /// Splitting is manually done
    Manual,
}

/// Constraints given to the [`Node`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct Constraint {
    /// Minimum width
    min_width:  u16,
    /// Minimum height
    min_height: u16,
}

/// Preselection area on the screen
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub(crate) struct Presel {
    /// Ratio of the preselection split
    split_ratio: f32,
    /// Direction of the split
    split_dir:   Direction,
    /// Window that receives the preselection overlay
    feedback:    Window,
}

impl Presel {
    /// Create a new [`Presel`]
    pub(crate) const fn new(ratio: f32) -> Self {
        Self {
            split_ratio: ratio,
            split_dir:   Direction::East,
            feedback:    x11rb::NONE,
        }
    }
}

/// Overall information about the X11 environment
#[derive(Debug, Clone)]
pub(crate) struct Coordinates {
    /// Current [`Monitor`]
    pub(crate) monitor: Monitor,
    /// Current [`Desktop`]
    pub(crate) desktop: Desktop,
    /// Current [`Node`]
    pub(crate) node:    Node,
}

/// History tracker about the X11 environment
#[derive(Debug, Clone)]
pub(crate) struct History {
    /// Current [`Monitor`], [`Desktop`], and [`Node`]
    loc:    Coordinates,
    /// Is this the latest item in [`History`]? TODO: what's this?
    latest: bool,
    /// Previous [`Monitor`], [`Desktop`], and [`Node`]
    prev:   Box<Self>,
    /// Next [`Monitor`], [`Desktop`], and [`Node`]
    next:   Box<Self>,
}

// =============================== Node ===============================

/// A single leaf in the tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Node {
    /// `id` of the [`Node`]
    id:           Xid,
    /// The type of [`Window`] split
    split_type:   SplitType,
    /// Ratio of the [`Split`]
    split_ratio:  f64,
    /// Preselection information
    presel:       Presel,
    /// [`Window`] dimensions
    rectangle:    Rectangle,
    /// [`Constraint`]s of this [`Node`]
    constraints:  Constraint,
    /// Is the current [`Node`] vacant?
    vacant:       bool,
    /// Is the current [`Node`] hidden?
    hidden:       bool,
    /// Is the current [`Node`] sticky?
    sticky:       bool,
    /// Is the current [`Node`] private?
    private:      bool,
    /// Is the current [`Node`] locked?
    locked:       bool,
    /// Is the current [`Node`] marked?
    marked:       bool,
    /// First child [`Node`] of current [`Node`]
    first_child:  usize,
    /// Second child [`Node`] of current [`Node`]
    second_child: usize,
    /// Parent [`Node`] of current [`Node`]
    parent:       Option<(usize, bool)>,
    /// Master [`Client`] running this [`Node`]
    client:       Client,
}

impl Node {
    /// Change the [`SplitType`] of the [`Node`]
    pub(crate) fn set_type(&mut self, type_: SplitType) {
        self.split_type = type_;
    }

    // /// Modify the [`Node`]'s constraints
    // fn update_constraints(&mut self) {
    //     if self.split_type == SplitType::Vertical {
    //         self.constraints.min_width =
    //             self.first_child.constraints.min_width +
    // self.second_child.constraints.min_width;     }
    // }

    /// Serialize [`Node`] into a `json` string
    pub(crate) fn query_node(&self) -> Result<String> {
        serde_json::to_string(&self).context("failed to serialize `Node` into json format")
    }

    /// Create a [`Node`] from a description
    pub(crate) fn from_desc(desc: &str, reference: &Coordinates, dest: &Coordinates) -> Self {
        // dest.node = NULL;

        todo!()
    }
}

// ============================== Desktop =============================

/// The current [`Desktop`]
///
/// One level above a [`Node`] and one level below a [`Monitor`]
#[derive(Debug, Clone)]
pub(crate) struct Desktop {
    /// Desktop's name
    name:         String,
    /// ID of the desktop
    id:           Xid,
    /// The layout of the desktop
    layout:       LayoutType,
    /// The user's layout of the desktop TODO: fill out
    user_layout:  LayoutType,
    /// Root [`Node`] of the desktop
    root:         Node,
    /// Focused [`Node`] of the desktop
    focus:        Node,
    /// Padding information about the desktop
    padding:      Padding,
    /// Current window gap settings
    window_gap:   isize,
    /// Current border width settings
    border_width: usize,

    /// Previous [`Desktop`]
    prev: Box<Self>,
    /// Next [`Desktop`]
    next: Box<Self>,
}

// ============================== Monitor =============================

/// The current [`Monitor`]
///
/// One level above a [`Desktop`] and one level below the overall tree
#[derive(Debug, Clone)]
pub(crate) struct Monitor {
    /// Monitor's name
    name:         String,
    /// ID of the monitor
    id:           Xid,
    /// `randr` ID of the monitor
    randr_id:     Output,
    /// Root [`Window`] of the monitor
    root:         Window,
    /// Is the monitor wired?
    wired:        bool,
    /// Number of sticky items
    sticky_count: usize,
    /// Padding information about the monitor
    padding:      Padding,
    /// Current window gap settings
    window_gap:   isize,
    /// Current border width settings
    border_width: usize,
    /// TODO: fill out
    rectangle:    Rectangle,
    /// Current [`Desktop`]
    desk:         Desktop,
    /// The first [`Desktop`] in the list
    desk_head:    Desktop,
    /// The last [`Desktop`] in the list
    desk_tail:    Desktop,

    /// Previous [`Monitor`]
    prev: Box<Self>,
    /// Next [`Monitor`]
    next: Box<Self>,
}
