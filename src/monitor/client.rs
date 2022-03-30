//! Metadata about an X-window

#![allow(clippy::missing_docs_in_private_items)]

use crate::{
    config::Config,
    core::{Identify, Pid, Window, WindowState, WindowType, Xid, MISSING_VALUE},
    geometry::{Extents, Padding, Point, Rectangle},
    stack::StackLayer,
    x::property::SizeHints,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use x11rb::properties::WmSizeHints;

// ============================= ClientState===========================

/// Current state of the [`Client`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum ClientState {
    /// Window is currently tiled
    Tiled,
    /// Window is currently pseudo-tiled
    PsuedoTiled,
    /// Window is currently floating
    Floating,
    /// Window is currently in fullscreen
    Fullscreen,
}

// ============================= IcccmProps ===========================

/// ICCCM window properties
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct IcccmProps {
    /// Request to take focus of the window
    take_focus:    bool,
    input_hint:    bool,
    /// Request to delete window
    delete_window: bool,
}

impl Default for IcccmProps {
    fn default() -> Self {
        Self {
            take_focus:    false,
            input_hint:    true,
            delete_window: false,
        }
    }
}

// =============================== Client =============================

/// Information about a top-level [`Window`]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Client {
    window:   Window,
    name:     String,
    class:    String,
    instance: String,

    border_width: usize,
    layer:        StackLayer,
    last_layer:   StackLayer,

    state:              ClientState,
    last_state:         ClientState,
    floating_rectangle: Rectangle,
    tiled_rectangle:    Rectangle,
    size_hints:         SizeHints,
    icccm_props:        IcccmProps,
    wm_flags:           WindowState,

    urgent: bool,
    shown:  bool,

    pid:  Option<Pid>,
    ppid: Option<Pid>,
}

impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.window == other.window
    }
}

// impl Default for Client {
//     fn default() -> Self {
//         Self {
//             window:             0,
//             name:               String::from(MISSING_VALUE),
//             class:              String::from(MISSING_VALUE),
//             instance:           String::from(MISSING_VALUE),
//             layer:              StackLayer::Normal,
//             last_layer:         StackLayer::Normal,
//             state:              ClientState::Tiled,
//             last_state:         ClientState::Tiled,
//             border_width:       1,
//             urgent:             false,
//             shown:              false,
//             floating_rectangle: Rectangle::default(),
//             tiled_rectangle:    Rectangle::default(),
//             icccm_props:        IcccmProps::default(),
//         }
//     }
// }
//
// impl Client {
//     pub(crate) fn new(config: &Config) -> Self {
//         Self {
//             window:       0,
//             name:         String::from(MISSING_VALUE),
//             class:        String::from(MISSING_VALUE),
//             instance:     String::from(MISSING_VALUE),
//             layer:        StackLayer::Normal,
//             last_layer:   StackLayer::Normal,
//             state:        ClientState::Tiled,
//             last_state:   ClientState::Tiled,
//             border_width: config.global.border_width,
//             urgent:       false,
//             shown:        false,
//         }
//     }
// }

// ============================== Client ==============================

use attr_rs::{attr_accessor, attr_reader, attr_writer};

// Easier to read with multiple calls, or separate lines?

/// Metadata about a X window.
///
/// Contains ICCCM and EWMH properties.
#[derive(Debug, Eq)]
#[attr_reader(
    window,
    frame,
    window_type,
    warp_point,
    active_region,
    previous_region,
    inner_region,
    free_region,
    tile_region,
    tree_region,
    pid,
    ppid,
    last_focused,
    managed_since
)]
#[attr_accessor(
    name,
    class,
    instance,
    context,
    workspace,
    frame_extents,
    size_hints,
    parent,
    leader,
    producer,
    mapped,
    managed,
    in_window,
    floating,
    fullscreen,
    iconified,
    disowned,
    sticky,
    invincible,
    urgent,
    consuming,
    producing
)]
pub(crate) struct Client1 {
    /// The name of the [`Client1`]
    name:     String,
    class:    String,
    instance: String,

    // border_width
    // state
    window:      Window,
    frame:       Window,
    context:     usize,
    workspace:   usize,
    window_type: WindowType,

    active_region:   Rectangle,
    previous_region: Rectangle,
    inner_region:    Rectangle,
    free_region:     Rectangle,
    tile_region:     Rectangle,
    tree_region:     Rectangle,

    frame_extents: Option<Extents>,
    size_hints:    Option<SizeHints>,
    warp_point:    Option<Point>,
    parent:        Option<Window>,
    children:      Vec<Window>,
    leader:        Option<Window>,
    producer:      Option<Window>,
    consumers:     Vec<Window>,

    focused:    bool,
    in_window:  bool,
    floating:   bool,
    fullscreen: bool,
    sticky:     bool,
    urgent:     bool,

    mapped:     bool,
    managed:    bool,
    iconified:  bool,
    disowned:   bool,
    invincible: bool,

    consuming: bool,
    producing: bool,

    last_focused:  SystemTime,
    managed_since: SystemTime,

    expected_unmap_count: u8,

    pid:  Option<Pid>,
    ppid: Option<Pid>,
}

impl Identify for Client1 {
    fn id(&self) -> Xid {
        self.window
    }
}

impl Client1 {
    /// Create a new [`Client1`]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        window: Window,
        frame: Window,
        name: String,
        class: String,
        instance: String,
        window_type: WindowType,
        pid: Option<Pid>,
        ppid: Option<Pid>,
    ) -> Self {
        Self {
            window,
            frame,
            name,
            class,
            instance,
            context: 0,
            workspace: 0,
            window_type,
            active_region: Rectangle::default(),
            previous_region: Rectangle::default(),
            inner_region: Rectangle::default(),
            free_region: Rectangle::default(),
            tile_region: Rectangle::default(),
            tree_region: Rectangle::default(),
            frame_extents: None,
            size_hints: None,
            warp_point: None,
            parent: None,
            children: vec![],
            leader: None,
            producer: None,
            consumers: vec![],
            focused: false,
            mapped: false,
            managed: true,
            in_window: false,
            floating: false,
            fullscreen: false,
            iconified: false,
            disowned: false,
            sticky: false,
            invincible: false,
            urgent: false,
            consuming: false,
            producing: true,
            last_focused: SystemTime::now(),
            managed_since: SystemTime::now(),
            expected_unmap_count: 0,
            pid,
            ppid,
        }
    }

    /// Return the current [`Window`] and the frame
    pub(crate) const fn windows(&self) -> (Window, Window) {
        (self.window, self.frame)
    }

    fn set_active_region(&mut self, active_region: &Rectangle) {
        self.previous_region = self.active_region;
        self.active_region = *active_region;
        self.set_inner_region(active_region);
    }

    fn set_inner_region(&mut self, active_region: &Rectangle) {
        self.inner_region = self.frame_extents.map_or_else(
            || {
                let mut inner_region = *active_region;

                inner_region.point.x = 0_i32;
                inner_region.point.y = 0_i32;

                inner_region
            },
            |frame_extents| {
                let mut inner_region = *active_region - frame_extents;

                inner_region.point.x = frame_extents.left as i32;
                inner_region.point.y = frame_extents.top as i32;

                inner_region.dimension.width =
                    active_region.dimension.width - frame_extents.left - frame_extents.right;
                inner_region.dimension.height =
                    active_region.dimension.height - frame_extents.top - frame_extents.bottom;

                inner_region
            },
        );
    }

    pub(crate) fn set_free_region(&mut self, free_region: &Rectangle) {
        if let Some(warp_point) = self.warp_point {
            if !free_region.is_inside(self.active_region.point + warp_point) {
                self.unset_warp_point();
            }
        }

        self.free_region = *free_region;
        self.set_active_region(free_region);
    }

    pub(crate) fn set_tile_region(&mut self, tile_region: &Rectangle) {
        if let Some(warp_point) = self.warp_point {
            if !tile_region.is_inside(self.active_region.point + warp_point) {
                self.unset_warp_point();
            }
        }

        self.tile_region = *tile_region;
        self.set_active_region(tile_region);
    }

    pub(crate) fn set_tree_region(&mut self, tree_region: &Rectangle) {
        if let Some(warp_point) = self.warp_point {
            if !tree_region.is_inside(self.active_region.point + warp_point) {
                self.unset_warp_point();
            }
        }

        self.tree_region = *tree_region;
        self.set_active_region(tree_region);
    }

    pub(crate) const fn frame_extents_unchecked(&self) -> Extents {
        if let Some(frame_extents) = self.frame_extents {
            frame_extents
        } else {
            Extents::EMPTY
        }
    }

    pub(crate) fn set_warp_point(&mut self, pointer_pos: Point) {
        let pointer_rpos = pointer_pos.relative(self.active_region.point);

        self.warp_point = self
            .active_region
            .is_inside(pointer_pos)
            .then(|| pointer_rpos);
    }

    pub(crate) fn unset_warp_point(&mut self) {
        self.warp_point = None;
    }

    pub(crate) fn add_child(&mut self, child: Window) {
        self.children.push(child);
    }

    pub(crate) fn remove_child(&mut self, child: Window) {
        if let Some(index) = self.children.iter().rposition(|c| *c == child) {
            self.children.remove(index);
        }
    }

    pub(crate) fn unset_producer(&mut self) {
        self.producer = None;
    }

    pub(crate) fn consumer_len(&self) -> usize {
        self.consumers.len()
    }

    pub(crate) fn add_consumer(&mut self, consumer: Window) {
        self.consumers.push(consumer);
    }

    pub(crate) fn remove_consumer(&mut self, consumer: Window) {
        if let Some(index) = self.consumers.iter().rposition(|c| *c == consumer) {
            self.consumers.remove(index);
        }
    }

    pub(crate) const fn is_free(&self) -> bool {
        self.floating || self.disowned || !self.managed
    }

    pub(crate) const fn is_focused(&self) -> bool {
        self.focused
    }

    pub(crate) fn set_focused(&mut self, focused: bool) {
        if focused {
            self.last_focused = SystemTime::now();
        }

        self.focused = focused;
    }

    pub(crate) fn expect_unmap(&mut self) {
        self.expected_unmap_count += 1;
    }

    pub(crate) const fn is_expecting_unmap(&self) -> bool {
        self.expected_unmap_count > 0
    }

    pub(crate) fn consume_unmap_if_expecting(&mut self) -> bool {
        let expecting = self.expected_unmap_count > 0;

        if expecting {
            self.expected_unmap_count -= 1;
        }

        expecting
    }
}

impl PartialEq for Client1 {
    fn eq(&self, other: &Self) -> bool {
        self.window == other.window
    }
}

// impl std::fmt::Debug for Client1 {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_struct("Client1")
//             .field("window", &Hex32(self.window))
//             .field("frame", &Hex32(self.frame))
//             .field("name", &self.name)
//             .field("class", &self.class)
//             .field("instance", &self.instance)
//             .field("context", &self.context)
//             .field("workspace", &self.workspace)
//             .field("window_type", &self.window_type)
//             .field("active_region", &self.active_region)
//             .field("previous_region", &self.previous_region)
//             .field("inner_region", &self.inner_region)
//             .field("free_region", &self.free_region)
//             .field("tile_region", &self.tile_region)
//             .field("tree_region", &self.tree_region)
//             .field("frame_extents", &self.frame_extents)
//             .field("size_hints", &self.size_hints)
//             .field("warp_point", &self.warp_point)
//             .field("parent", &self.parent.map(|parent| Hex32(parent)))
//             .field(
//                 "children",
//                 &self
//                     .children
//                     .iter()
//                     .map(|&child| Hex32(child))
//                     .collect::<Vec<Hex32>>(),
//             )
//             .field("leader", &self.leader)
//             .field("producer", &self.producer)
//             .field("consumers", &self.consumers)
//             .field("focused", &self.focused)
//             .field("mapped", &self.mapped)
//             .field("managed", &self.managed)
//             .field("in_window", &self.in_window)
//             .field("floating", &self.floating)
//             .field("fullscreen", &self.fullscreen)
//             .field("iconified", &self.iconified)
//             .field("disowned", &self.disowned)
//             .field("sticky", &self.sticky)
//             .field("invincible", &self.invincible)
//             .field("urgent", &self.urgent)
//             .field("consuming", &self.consuming)
//             .field("pid", &self.pid)
//             .field("ppid", &self.ppid)
//             .field("last_focused", &self.last_focused)
//             .field("managed_since", &self.managed_since)
//             .field("expected_unmap_count", &self.expected_unmap_count)
//             .finish()
//     }
// }

mod tests {
    use super::{Client1, WindowType};

    #[test]
    fn attr_accesor() {
        let mut client = Client1::new(
            1,
            1,
            String::from("window1"),
            String::from("class1"),
            String::from("instance1"),
            WindowType::Desktop,
            None,
            None,
        );

        assert_eq!(client.get_name(), &String::from("window1"));
        assert_eq!(client.get_window(), &1);

        client.set_class(String::from("another_class"));
        assert_eq!(client.get_class(), &String::from("another_class"));
    }
}
