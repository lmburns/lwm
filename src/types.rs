//! Base types used throughout [`lwm`]

#![allow(clippy::missing_docs_in_private_items)]

use crate::{geometry::Rectangle, input::ModMask, xconnection::Atoms};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{
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

/// State of the current window.
///
/// More information can be found in the [X11 documentation][1]
///
/// [1]: https://specifications.freedesktop.org/wm-spec/wm-spec-latest.html#idm45381392044896
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

// ========================== Icccm Values =========================

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

impl From<IcccmWindowState> for u32 {
    fn from(u: IcccmWindowState) -> Self {
        match u {
            IcccmWindowState::Withdrawn => 0,
            IcccmWindowState::Normal => 1,
            IcccmWindowState::Iconic => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct IcccmProps {
    /// Request to take focus of the window
    take_focus:    bool,
    input_hint:    bool,
    /// Request to delete window
    delete_window: bool,
}

// ============================== Unused ==============================
// ====================================================================

/// The type of [`Window`] split
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SplitType {
    /// Window is split with the axis of the split going from East to West
    Horizontal,
    /// Window is split with the axis of the split going from North to South
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SplitMode {
    Automatic,
    Manual,
}

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
pub(crate) enum ClientState {
    Tiled,
    PsuedoTiled,
    Floating,
    Fullscreen,
}

/// The type of [`Window`] in the [`StackingList`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StackLayer {
    /// Window is `Below` another
    Below,
    /// Window is focused
    Normal,
    /// Window is `Above` another
    Above,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AlterState {
    Toggle,
    Set,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CycleDir {
    Next,
    Prev,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CirculateDir {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum HistoryDir {
    Older,
    Newer,
}

/// A standard direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ResizeHandle {
    Left,
    Top,
    Right,
    Bottom,
    TopLeft,
    TopRight,
    BottomRight,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Layout {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NodeSelect {
    automatic:     bool,
    focused:       bool,
    active:        bool,
    local:         bool,
    leaf:          bool,
    window:        bool,
    tiled:         bool,
    pseudo_tiled:  bool,
    floating:      bool,
    fullscreen:    bool,
    hidden:        bool,
    sticky:        bool,
    private:       bool,
    locked:        bool,
    marked:        bool,
    urgent:        bool,
    same_class:    bool,
    descendant_of: bool,
    ancestor_of:   bool,
    below:         bool,
    normal:        bool,
    above:         bool,
    horizontal:    bool,
    vertical:      bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DesktopSelect {
    occupied:     bool,
    focused:      bool,
    active:       bool,
    urgent:       bool,
    local:        bool,
    tiled:        bool,
    monocle:      bool,
    user_tiled:   bool,
    user_monocle: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MonitorSelect {
    occupied: bool,
    focused:  bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Client {
    class_name:         String,
    instance_name:      String,
    name:               String,
    border_width:       usize,
    state:              ClientState,
    last_state:         ClientState,
    layer:              StackLayer,
    last_layer:         StackLayer,
    floating_rectangle: Rectangle,
    tilde_rectangle:    Rectangle,
    size_hints:         WmSizeHints,
    icccm_props:        IcccmProps,
    wm_flags:           WindowState,

    urgent: bool,
    shown:  bool,

    pid:  Option<Pid>,
    ppid: Option<Pid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Presel {
    split_ratio: bool,
    split_dir:   bool,
    feedback:    Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Constraint {
    min_width:  u16,
    min_height: u16,
}

#[derive(Debug, Clone)]
pub(crate) struct Node {
    /// `id` of the [`Node`]
    id:           Xid,
    split_type:   SplitType,
    /// Ratio of the [`Split`]
    split_ratio:  f64,
    /// Preselection information
    presel:       Presel,
    /// Window dimensions
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
    first_child:  Box<Self>,
    /// Second child [`Node`] of current [`Node`]
    second_child: Box<Self>,
    /// Parent [`Node`] of current [`Node`]
    parent:       Box<Self>,
    /// Master [`Client`] running this [`Node`]
    client:       Client,
}

/// Padding around a window
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Padding {
    pub(crate) top:    u32,
    pub(crate) right:  u32,
    pub(crate) bottom: u32,
    pub(crate) left:   u32,
}

impl Padding {
    /// Create a new [`Padding`]
    pub(crate) const fn new(top: u32, right: u32, bottom: u32, left: u32) -> Self {
        Self { top, right, bottom, left }
    }
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
pub(crate) struct Desktop {
    name:         String,
    id:           Xid,
    layout:       Layout,
    user_layout:  Layout,
    root:         Node,
    focus:        Node,
    padding:      Padding,
    window_gap:   isize,
    border_width: usize,
    // prev:         Box<Self>,
    // next:         Box<Self>,
}

#[derive(Debug, Clone)]
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
    prev:         Box<Self>,
    next:         Box<Self>,
}

#[derive(Debug, Clone)]
pub(crate) struct Coordinates {
    monitor: Monitor,
    desktop: Desktop,
    node:    Node,
}

#[derive(Debug, Clone)]
pub(crate) struct History {
    loc:    Coordinates,
    latest: bool,
    prev:   Box<Self>,
    next:   Box<Self>,
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

#[derive(Debug, Clone)]
pub(crate) struct SubscriberList {
    // file: Stream,
    fifo_path: String,
    field:     usize,
    count:     usize,
    prev:      Box<Self>,
    next:      Box<Self>,
}

#[derive(Debug, Clone)]
pub(crate) struct Rule {
    class_name:    String,
    instance_name: String,
    name:          String,
    effect:        String,
    one_shot:      bool,
    prev:          Box<Self>,
    next:          Box<Self>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuleConsequence {
    class_name:    String,
    instance_name: String,
    name:          String,
    monitor_desc:  String,
    desktop_desc:  String,
    node_desc:     String,
    split_dir:     Direction,
    split_ratio:   f64,
    layer:         StackLayer,
    state:         ClientState,
    hidden:        bool,
    sticky:        bool,
    private:       bool,
    locked:        bool,
    marked:        bool,
    center:        bool,
    follow:        bool,
    manage:        bool,
    focus:         bool,
    border:        bool,
    rect:          Rectangle,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingRule {
    fd:         usize,
    win:        Window,
    csq:        RuleConsequence,
    event_head: EventQueue,
    event_tail: EventQueue,
    prev:       Box<Self>,
    next:       Box<Self>,
}

// ====================================================================
// ====================================================================

use x11rb::properties::{AspectRatio, WmSizeHints};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Position {
    pub(crate) x: i32,
    pub(crate) y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Ratio {
    pub(crate) numerator:   i32,
    pub(crate) denominator: i32,
}

#[derive(Debug, Copy, Clone, PartialOrd)]
pub(crate) struct SizeHints {
    /// User flags
    pub(crate) by_user:          bool,
    /// User-specified size
    pub(crate) pos:              Option<Position>,
    /// Program-specified minimum width
    pub(crate) min_width:        Option<u32>,
    /// Program-specified minimum height
    pub(crate) min_height:       Option<u32>,
    /// Program-specified maximum width
    pub(crate) max_width:        Option<u32>,
    /// Program-specified maximum height
    pub(crate) max_height:       Option<u32>,
    /// Program-specified base width
    pub(crate) base_width:       Option<u32>,
    /// Program-specified base height
    pub(crate) base_height:      Option<u32>,
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

// ====================================================================
// ====================================================================
