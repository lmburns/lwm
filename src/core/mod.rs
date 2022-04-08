//! Base types used throughout [`lwm`]

#![allow(clippy::missing_docs_in_private_items)]

pub(crate) mod decoration;

use crate::{
    geometry::{Dimension, Point, Ratio, Rectangle},
    tree::Node,
    x::{input::ModMask, xconnection::Atoms},
};
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{
    cmp,
    collections::HashMap,
    fmt,
    ops::{Add, Div, Mul, Sub},
};
use x11rb::{
    protocol::{xproto, Event},
    rust_connection::Stream,
};

// Re-export
pub(crate) use x11rb::protocol::{
    randr::Output,
    xproto::{Atom, AtomEnum, Button, Keycode, Window},
};

/// Type alias used for syntax compatibility
pub(crate) type Pid = u32;
/// Type alias used for syntax compatibility
pub(crate) type Xid = u32;
/// Type alias for a given index
pub(crate) type Idx = usize;

/// Trait to return the [`Window`]'s ID
pub(crate) trait Identify: PartialEq {
    fn id(&self) -> Xid;
}

impl Identify for Window {
    fn id(&self) -> Xid {
        *self
    }
}

/// Default string for missing values
pub(crate) const MISSING_VALUE: &str = "N/A";
/// Maximum number of window manager states
pub(crate) const MAX_WM_STATES: u8 = 4;
/// Height of the title bar
pub(crate) const TITLEBAR_HEIGHT: u16 = 20;
/// Button (mouse) index used to drag a window
pub(crate) const DRAG_BUTTON: Button = 1;

/// Window manager's name
#[macro_export]
macro_rules! WM_NAME (
    () => { "lwm" };
);

/// Window manager's class name
pub(crate) const WM_CLASS_NAME: &str = "Lwm";
/// Meta window's class name
pub(crate) const META_WINDOW_IC: Lazy<String> = Lazy::new(|| format!("wm\0{}", WM_CLASS_NAME));
/// Root window's class name
pub(crate) const ROOT_WINDOW_IC: Lazy<String> = Lazy::new(|| format!("root\0{}", WM_CLASS_NAME));

// ============================== XWindow =============================
// ====================================================================

/// Representation of an X-window with its geometrical data
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct XWindow {
    /// The ID of the window
    pub(crate) id:   Window,
    /// Window size
    pub(crate) rect: Rectangle,
}

impl XWindow {
    /// Create a new [`XWindow`]
    pub(crate) const fn new(id: Window, rect: Rectangle) -> Self {
        Self { id, rect }
    }

    /// Create a blank window
    pub(crate) fn zeroed() -> Self {
        Self::default()
    }
}

impl PartialEq for XWindow {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl From<Window> for XWindow {
    fn from(w: Window) -> Self {
        Self {
            id:   w,
            rect: Rectangle::zeroed(),
        }
    }
}

// =========================== Window Edges ===========================
// ====================================================================

/// An edge of the screen
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Clone, Copy, Hash)]
pub(crate) enum Edge {
    /// The left edge
    Left,
    /// The right edge
    Right,
    /// The top edge
    Top,
    /// The bottom edge
    Bottom,
}

/// A corner of a window
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Clone, Copy)]
pub(crate) enum Corner {
    /// The top-left corner
    TopLeft,
    /// The top-right corner
    TopRight,
    /// The bottom-left corner
    BottomLeft,
    /// The bottom-right corner
    BottomRight,
}

// impl Corner {
//     /// Obtain the relative location of a corner for a given client window.
//     fn relative(&self, st: &ClientState) -> (i16, i16) {
//         match self {
//             Self::TopLeft => (0, 0),
//             Self::TopRight => (st.width as i16, 0),
//             Self::BottomLeft => (0, st.height as i16),
//             Self::BottomRight => (st.width as i16, st.height as i16),
//         }
//     }
// }

/// Action of a mouse moving edge/corner of a window
#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub(crate) enum DragType {
    /// A moving (edge) drag
    Edge(Edge),
    /// A resizing (corner) drag
    Corner(Corner),
}

impl DragType {
    /// Is the type of drag moving a top [`Corner`] or [`Edge`]?
    pub(crate) fn is_top_drag(self) -> bool {
        self == Self::Edge(Edge::Top)
            || self == Self::Corner(Corner::TopLeft)
            || self == Self::Corner(Corner::TopRight)
    }

    /// Is the type of drag moving a right [`Corner`] or [`Edge`]?
    pub(crate) fn is_right_drag(self) -> bool {
        self == Self::Edge(Edge::Right)
            || self == Self::Corner(Corner::TopRight)
            || self == Self::Corner(Corner::BottomRight)
    }
}

/// State of a [`Window`](xproto::Window) drag
#[derive(Debug, Clone)]
pub(crate) struct Drag {
    /// Type of drag
    r#type: DragType,
    /// Window that is being dragged
    window: xproto::Window,
    /// X-position of the pointer relative to a corner of the window
    x:      i16,
    /// Y-position of the pointer relative to a corner of the window
    y:      i16,
}

// ========================== WindowType ===========================

/// Window type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum WindowType {
    /// Window that appears below a text field with a list of suggestions
    Combo,
    /// Single window containing desktop icons same dimensions as the screen
    Desktop,
    /// A dialog window
    Dialog,
    /// Window that is being dragged from one location to another
    DND,
    /// Dock or panel, keep such windows on top of others
    Dock,
    /// Window that appears from a menubar
    DropdownMenu,
    /// Pinnable menu windows torn-off from the main window
    Menu,
    /// A normal, top-level window
    Normal,
    /// A notification window
    Notification,
    /// Window that usually appears from a right-click
    PopupMenu,
    /// A splash screen, a.k.a., an application startup screen
    Splash,
    /// Toolbar torn-off from the main window
    Toolbar,
    /// Short piece of explanatory text that appears after a mouse hover
    ToolTip,
    /// Small persistent utility window (e.g., pallete or toolbox)
    Utility,
}

impl WindowType {
    /// Convert [`Atoms`] to a [`WindowType`]
    pub(crate) fn from_atoms(atom: &Atoms, u: Atom) -> Result<Self> {
        match atom {
            z @ Atoms { .. } if z._NET_WM_WINDOW_TYPE_COMBO == u => Ok(Self::Combo),
            z @ Atoms { .. } if z._NET_WM_WINDOW_TYPE_DESKTOP == u => Ok(Self::Desktop),
            z @ Atoms { .. } if z._NET_WM_WINDOW_TYPE_DIALOG == u => Ok(Self::Dialog),
            z @ Atoms { .. } if z._NET_WM_WINDOW_TYPE_DND == u => Ok(Self::DND),
            z @ Atoms { .. } if z._NET_WM_WINDOW_TYPE_DOCK == u => Ok(Self::Dock),
            z @ Atoms { .. } if z._NET_WM_WINDOW_TYPE_DROPDOWN_MENU == u => Ok(Self::DropdownMenu),
            z @ Atoms { .. } if z._NET_WM_WINDOW_TYPE_MENU == u => Ok(Self::Menu),
            z @ Atoms { .. } if z._NET_WM_WINDOW_TYPE_NORMAL == u => Ok(Self::Normal),
            z @ Atoms { .. } if z._NET_WM_WINDOW_TYPE_NOTIFICATION == u => Ok(Self::Notification),
            z @ Atoms { .. } if z._NET_WM_WINDOW_TYPE_POPUP_MENU == u => Ok(Self::PopupMenu),
            z @ Atoms { .. } if z._NET_WM_WINDOW_TYPE_SPLASH == u => Ok(Self::Splash),
            z @ Atoms { .. } if z._NET_WM_WINDOW_TYPE_TOOLBAR == u => Ok(Self::Toolbar),
            z @ Atoms { .. } if z._NET_WM_WINDOW_TYPE_TOOLTIP == u => Ok(Self::ToolTip),
            z @ Atoms { .. } if z._NET_WM_WINDOW_TYPE_UTILITY == u => Ok(Self::Utility),
            other => Err(anyhow!("invalid window type: {}", u)),
        }
    }

    /// Create a [`HashMap`] of [`Atom`]s and [`WindowType`]s
    pub(crate) fn to_hashmap(a: &Atoms) -> HashMap<Atom, Self> {
        maplit::hashmap! {
            a._NET_WM_WINDOW_TYPE_COMBO         => Self::Combo,
            a._NET_WM_WINDOW_TYPE_DESKTOP       => Self::Desktop,
            a._NET_WM_WINDOW_TYPE_DIALOG        => Self::Dialog,
            a._NET_WM_WINDOW_TYPE_DND           => Self::DND,
            a._NET_WM_WINDOW_TYPE_DOCK          => Self::Dock,
            a._NET_WM_WINDOW_TYPE_DROPDOWN_MENU => Self::DropdownMenu,
            a._NET_WM_WINDOW_TYPE_MENU          => Self::Menu,
            a._NET_WM_WINDOW_TYPE_NORMAL        => Self::Normal,
            a._NET_WM_WINDOW_TYPE_NOTIFICATION  => Self::Notification,
            a._NET_WM_WINDOW_TYPE_POPUP_MENU    => Self::PopupMenu,
            a._NET_WM_WINDOW_TYPE_SPLASH        => Self::Splash,
            a._NET_WM_WINDOW_TYPE_TOOLBAR       => Self::Toolbar,
            a._NET_WM_WINDOW_TYPE_TOOLTIP       => Self::ToolTip,
            a._NET_WM_WINDOW_TYPE_UTILITY       => Self::Utility
        }
    }
}

impl fmt::Display for WindowType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// ========================== WindowState ==========================

// NOTE: bspwm Equivalent to `wm_flags_t`
/// State of the current window.
///
/// More information can be found in the [X11 documentation][1]
///
/// [1]: https://specifications.freedesktop.org/wm-spec/wm-spec-latest.html#idm45381392044896
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum WindowState {
    /// Window is on-top of or above another
    Above,
    /// Window is below another
    Below,
    /// Window is demanding attention (e.g., prompt displays)
    DemandsAttention,
    /// Window takes up as much of the screen as possible
    Fullscreen,
    /// Window is minimized or hidden
    Hidden,
    /// Window takes up as much area as possible in the horizontal direction
    MaximizedHorz,
    /// Window takes up as much area as possible in the vertical direction
    MaximizedVert,
    /// Top-level window may be closed before client finishes (e.g., dialog box)
    Modal,
    /// Window only shows the titlebar (a.k.a., rollup)
    Shaded,
    /// Miniature view of the desktop(s) that allows manipulation with these
    SkipPager,
    /// List of buttons labeled with titles and icons
    SkipTaskbar,
    /// Window which is stuck or pinned in the same position
    Sticky,
}

impl WindowState {
    /// Convert [`Atoms`] to a [`WindowState`]
    pub(crate) fn from_atoms(atom: &Atoms, u: Atom) -> Result<Self> {
        match atom {
            z @ Atoms { .. } if z._NET_WM_STATE_ABOVE == u => Ok(Self::Above),
            z @ Atoms { .. } if z._NET_WM_STATE_BELOW == u => Ok(Self::Below),
            z @ Atoms { .. } if z._NET_WM_STATE_DEMANDS_ATTENTION == u => Ok(Self::DemandsAttention),
            z @ Atoms { .. } if z._NET_WM_STATE_FULLSCREEN == u => Ok(Self::Fullscreen),
            z @ Atoms { .. } if z._NET_WM_STATE_HIDDEN == u => Ok(Self::Hidden),
            z @ Atoms { .. } if z._NET_WM_STATE_MAXIMIZED_HORZ == u => Ok(Self::MaximizedHorz),
            z @ Atoms { .. } if z._NET_WM_STATE_MAXIMIZED_VERT == u => Ok(Self::MaximizedVert),
            z @ Atoms { .. } if z._NET_WM_STATE_MODAL == u => Ok(Self::Modal),
            z @ Atoms { .. } if z._NET_WM_STATE_SHADED == u => Ok(Self::Shaded),
            z @ Atoms { .. } if z._NET_WM_STATE_SKIP_PAGER == u => Ok(Self::SkipPager),
            z @ Atoms { .. } if z._NET_WM_STATE_SKIP_TASKBAR == u => Ok(Self::SkipTaskbar),
            z @ Atoms { .. } if z._NET_WM_STATE_STICKY == u => Ok(Self::Sticky),
            other => Err(anyhow!("invalid window state: {}", u)),
        }
    }

    /// Create a [`HashMap`] of [`Atom`]s and [`WindowType`]s
    pub(crate) fn to_hashmap(a: &Atoms) -> HashMap<Atom, Self> {
        maplit::hashmap! {
            a._NET_WM_STATE_ABOVE             => Self::Above,
            a._NET_WM_STATE_BELOW             => Self::Below,
            a._NET_WM_STATE_DEMANDS_ATTENTION => Self::DemandsAttention,
            a._NET_WM_STATE_FULLSCREEN        => Self::Fullscreen,
            a._NET_WM_STATE_HIDDEN            => Self::Hidden,
            a._NET_WM_STATE_MAXIMIZED_HORZ    => Self::MaximizedHorz,
            a._NET_WM_STATE_MAXIMIZED_VERT    => Self::MaximizedVert,
            a._NET_WM_STATE_MODAL             => Self::Modal,
            a._NET_WM_STATE_SHADED            => Self::Shaded,
            a._NET_WM_STATE_SKIP_PAGER        => Self::SkipPager,
            a._NET_WM_STATE_SKIP_TASKBAR      => Self::SkipTaskbar,
            a._NET_WM_STATE_STICKY            => Self::Sticky,
        }
    }
}

impl fmt::Display for WindowState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// =========================== WindowMap ===========================

/// Possible values for the state of the window mapping
///
/// See the [X11 docs][1] for more information.
// [1]: https://www.x.org/releases/X11R7.6/doc/libX11/specs/libX11/libX11.html#Mapping_Windows_
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum WindowMap {
    /// Window is unmapped, meaning  `map_window` has not be called to it
    /// Retains stacking position when the window is unmapped
    Unmapped,
    /// May not be visible because:
    ///  - Window is mapped but an ancestor is not
    ///  - Window is obscured by another non-transparent window
    ///  - Window is entirely clipped by an ancestor
    /// It becomes viewable once the ancestor is mapped
    Unviewable,
    /// All of the window's ancestors are mapped and the view isn't obscured
    Viewable,
}

impl WindowMap {
    /// Convert an [`Atom`] to a [`WindowMap`]
    pub(crate) fn from_atoms(u: Atom) -> Result<Self> {
        use xproto::MapState;
        match u {
            s if s == u32::from(MapState::UNMAPPED) => Ok(Self::Unmapped),
            s if s == u32::from(MapState::UNVIEWABLE) => Ok(Self::Unviewable),
            s if s == u32::from(MapState::VIEWABLE) => Ok(Self::Viewable),
            other => Err(anyhow!("invalid window map: {}", u)),
        }
    }
}

impl fmt::Display for WindowMap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// ========================== WindowClass ==========================

// NOTE: x11rb::properties::WmClass
/// Equivalent to [`WindowClass`](xproto::WindowClass)
///
/// See [X11 documentation][1] for more information
///
/// [1]: https://www.x.org/releases/X11R7.6/doc/libX11/specs/libX11/libX11.html#glossary:InputOnly_window
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum WindowClass {
    /// Class is taken from the parent resulting in `InputOnly` or `InputOutput`
    CopyFromParent,
    /// Invisible and can only be used to control things. For example a cursor,
    /// event generation, or grabbing. Cannot have `InputOutput` as an inferior
    ///  - Event supression mask (supresses propagation of events from children)
    InputOnly,
    /// A normal kind of window that is used for both input and output. It can
    /// have both `InputOutput` and `InputOnly` windows as inferiors
    ///  - Border width of 0+ pixels
    ///  - Optional background
    ///  - Event supression mask (supresses propogation of events from children)
    ///  - Has a property list
    InputOutput,
}

impl WindowClass {
    // Convert an [`Atom`] to a [`WindowClass`]
    pub(crate) fn from_atoms(u: Atom) -> Result<Self> {
        use xproto::WindowClass as XWindowClass;
        match u {
            s if s == u32::from(XWindowClass::COPY_FROM_PARENT) => Ok(Self::CopyFromParent),
            s if s == u32::from(XWindowClass::INPUT_ONLY) => Ok(Self::InputOnly),
            s if s == u32::from(XWindowClass::INPUT_OUTPUT) => Ok(Self::InputOutput),
            other => Err(anyhow!("invalid window class: {}", u)),
        }
    }
}

impl TryFrom<xproto::WindowClass> for WindowClass {
    type Error = anyhow::Error;

    fn try_from(u: xproto::WindowClass) -> Result<Self, Self::Error> {
        use xproto::WindowClass as XWindowClass;
        match u {
            XWindowClass::COPY_FROM_PARENT => Ok(Self::CopyFromParent),
            XWindowClass::INPUT_OUTPUT => Ok(Self::InputOutput),
            XWindowClass::INPUT_ONLY => Ok(Self::InputOnly),
            other => Err(anyhow!("invalid window class: {:?}", u)),
        }
    }
}

impl fmt::Display for WindowClass {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// ============================== Unused ==============================
// ====================================================================

/// Insertion scheme used when the insertion point is in automatic mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AutomaticScheme {
    #[serde(alias = "longest-side", alias = "longest_side")]
    LongestSide,
    Alternate,
    Spiral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AlterState {
    Toggle,
    Set,
}

/// Window cycle direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CycleDir {
    /// Cycle to the next item
    Next,
    /// Cycle to the previous item
    Prev,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum HistoryDir {
    Older,
    Newer,
}

/// A standard direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum Direction {
    /// North or above relative to something else
    North,
    /// South or below relative to something else
    South,
    /// East or left relative something else
    East,
    /// West or right relative something else
    West,
}

/// Part of the [`Window`] that is being moved to cause a resize
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ResizeHandle {
    /// Moving left side of window
    Left,
    /// Moving top side of window
    Top,
    /// Moving right side of window
    Right,
    /// Moving bottom side of window
    Bottom,
    /// Corner resize of top-left corner
    TopLeft,
    /// Corner resize of top-right corner
    TopRight,
    /// Corner resize of bottom-right corner
    BottomRight,
    /// Corner resize of bottom-left corner
    BottomLeft,
}

/// Action performed when the [`ModMask`] and [`Button`] are held
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PointerAction {
    /// No action is performed
    None,
    /// Window becomes focused
    Focus,
    /// Window is moved
    Move,
    /// Side of the window is used to resize
    #[serde(alias = "resize-side", alias = "resize_side")]
    ResizeSide,
    /// Corner of the window is used to resize
    #[serde(alias = "resize-corner", alias = "resize_corner")]
    ResizeCorner,
}

/// The layout of the current [`Window`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LayoutType {
    Tiled,
    Monocle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Flip {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AreaPeak {
    Biggest,
    Smallest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum StateTransition {
    Enter,
    Exit,
}

/// Tightness of algorithm used to decide whether a [`Window`] is on the
/// [`Direction`] side of another [`Window`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tightness {
    /// A low tightness of the algorithm
    Low,
    /// A high tightness of the algorithm
    High,
}

// Which child should a new window be attached when adding a window on a single
// window tree in `automatic` mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ChildPolarity {
    /// First child
    First,
    /// Second child
    Second,
}

#[derive(Debug, Clone)]
pub(crate) struct StackingList {
    node: Node,
    prev: Box<Self>,
    next: Box<Self>,
}

#[derive(Debug, Clone)]
pub(crate) struct EventQueue {
    event: Event,
    prev:  Box<Self>,
    next:  Box<Self>,
}

// ====================================================================
// ====================================================================

// ============================== Strut ===============================

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) struct Strut {
    pub(crate) window: Window,
    pub(crate) width:  u32,
}

impl Strut {
    pub(crate) const fn new(window: Window, width: u32) -> Self {
        Self { window, width }
    }
}

impl PartialOrd for Strut {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Strut {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.width.cmp(&self.width)
    }
}

// ============================== Panel ===============================

// #[derive(Debug, Copy, Clone, PartialEq, Eq)]
// pub(crate) struct Panel {
//     window: Window,
//     strut:  WMStrut,
// }

// impl Panel {
//     pub(crate) fn new(window: Window, strut: WmStrut) -> Self {
//         Self { window, strut }
//     }
// }

// #[derive(Debug, PartialEq, Default, Copy, Clone, Eq)]
// pub(crate) struct WMStrut {
//     left:   u32,
//     right:  u32,
//     top:    u32,
//     bottom: u32,
// }
//
// impl PartialOrd for Panel {
//     fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
//         Some(self.cmp(other))
//     }
// }
//
// impl Ord for Panel {
//     fn cmp(&self, other: &Self) -> cmp::Ordering {
//         other.strut.cmp(&self.strut)
//     }
// }

// ============================= StackMode =============================

#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub(crate) enum StackMode {
    Above,
    Below,
    // TopIf,
    // BottomIf,
    // Opposite,
}

// ========================== Motion Recorder ==========================

#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub(crate) struct MotionRecorder {
    pub(crate) id:       Xid,
    pub(crate) sequence: u16,
    pub(crate) enabled:  bool,
}

impl MotionRecorder {
    /// Create a new [`MotionRecorder`]
    pub(crate) const fn new(id: Xid) -> Self {
        Self {
            id,
            sequence: 0,
            enabled: false,
        }
    }
}
