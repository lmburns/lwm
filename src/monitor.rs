//! Interactions with the [`Monitor`] struct

use crate::{
    geometry::Rectangle,
    types::{Desktop, Output, Padding, Window, Xid},
};
use anyhow::{Context, Result};

/// Represents a monitor connected to the window manager
#[derive(Debug, Clone)]
#[allow(clippy::missing_docs_in_private_items)]
pub(crate) struct Monitor {
    name:         String,
    id:           Xid,
    randr_id:     Output,
    root:         Window,
    wired:        bool,
    padding:      Padding,
    sticky_count: usize,
    window_gap:   isize,
    border_width: usize,
    rectangle:    Rectangle,
    desk:         Desktop,
    desk_head:    Desktop,
    desk_tail:    Desktop,
    // prev:         Box<Self>,
    // next:         Box<Self>,
}
