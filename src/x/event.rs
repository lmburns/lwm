//! X11 Events

use crate::{
    core::{Atom, StackMode, Window, XWindow},
    geometry::{Dimension, Point, Rectangle},
    x::input::{Button, Keycode},
};

// ============================== XEvent ==============================

/// Low-level wrapper around X-server events
///
/// Translated to EventActions by `WindowManager`
#[derive(Debug, Clone)]
pub(crate) enum XEvent {
    /// Notification that a client has changed its configuration
    ConfigureNotify(ConfigureEvent),
    /// Request for configuration from a client
    ConfigureRequest(ConfigureRequestData),
    /// A Client is requesting to be mapped
    MapRequest(Window, bool), // bool: override_redirect
    /// A Client has mapped a window
    MapNotify(Window),
    /// A Client has unmapped a window
    UnmapNotify(Window),
    /// A Client has destroyed a window
    DestroyNotify(Window),
    /// The pointer has entered a window
    ///
    /// The bool is whether the pointer is grabbed
    EnterNotify(PointerEvent, bool),
    /// The pointer has left a window
    ///
    /// The bool is whether the pointer is grabbed
    LeaveNotify(PointerEvent, bool),
    /// A window was reparented
    ReparentNotify(ReparentEvent),
    /// A window property was changed
    PropertyNotify(PropertyEvent),
    /// A key combination was pressed
    KeyPress(Window, KeypressEvent),
    /// A key combination was released
    //? does this need any more information?
    KeyRelease,
    /// A mouse button was pressed
    MouseEvent(MouseEvent),
    /// A client message was received
    ClientMessage(ClientMessageEvent),
    /// Received a randr notification
    RandrNotify,
    /// Received a randr screen change notify event
    ScreenChange,
    /// Unknown event type, used as a catchall for events not tracked by
    /// toaruwm
    Unknown(u8),
}

/// Data associated with a configure event
#[derive(Debug, Clone, Copy)]
pub(crate) struct ConfigureEvent {
    /// The window associated with the event
    pub(crate) id:      Window,
    /// The new geometry requested by the window
    pub(crate) geom:    Rectangle,
    /// Is the window the root window?
    pub(crate) is_root: bool,
}

/// Data associated with a configure request
#[derive(Debug, Clone, Copy)]
pub(crate) struct ConfigureRequestData {
    /// The window associated with the event
    pub(crate) id:         Window,
    /// The parent window of id
    pub(crate) parent:     Window,
    /// Sibling window of id. Used if stack_mode is set
    pub(crate) sibling:    Option<Window>,
    /// X coordinate to configure to
    pub(crate) x:          Option<i32>,
    /// Y coordinate to configure to
    pub(crate) y:          Option<i32>,
    /// Window height to configure to
    pub(crate) height:     Option<u32>,
    /// Window width to configure to
    pub(crate) width:      Option<u32>,
    /// Stack mode to configure to
    pub(crate) stack_mode: Option<StackMode>,
    /// If the window to configure is root
    pub(crate) is_root:    bool,
}

/// Data associated with a reparent event
#[derive(Debug, Clone, Copy)]
pub(crate) struct ReparentEvent {
    /// The event window
    pub(crate) event:    Window,
    /// The parent window
    pub(crate) parent:   Window,
    /// The child of the parent window
    pub(crate) child:    Window,
    /// Whether the child window is override-redirect
    pub(crate) over_red: bool,
}

/// Data associated with a pointer change event (Enter, Leave)
#[derive(Debug, Clone, Copy)]
pub(crate) struct PointerEvent {
    /// The id of the event window
    pub(crate) id:  Window,
    /// The absolute position of the pointer (relative to root)
    pub(crate) abs: Point,
    /// The relative position of the pointer to the event window
    pub(crate) rel: Point,
}

/// Data associated with a property change event
#[derive(Debug, Clone, Copy)]
pub(crate) struct PropertyEvent {
    /// The window associated with the event
    pub(crate) id:      Window,
    /// The atom representing the change
    pub(crate) atom:    Atom,
    /// The time of event
    pub(crate) time:    u32,
    /// Whether the property was deleted
    pub(crate) deleted: bool,
}

/// Data associated with a key press event
#[derive(Debug, Clone, Copy)]
pub(crate) struct KeypressEvent {
    /// The state of modifier keys was active at the time
    pub(crate) mask:    u16,
    /// The keycode of the key pressed
    pub(crate) keycode: u8,
}

/// Data associated with a button press event
#[derive(Debug, Clone)]
pub(crate) struct MouseEvent {
    /// The window the pointer was on when the button was pressed
    pub(crate) id:       Window,
    /// The location of the pointer when the button was pressed
    pub(crate) location: Point,
    // /// The state of the buttons and the movement type
    // pub(crate) state:    Mousebind,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ClientMessageEvent {
    pub(crate) window: Window,
    pub(crate) data:   ClientMessageData,
    pub(crate) type_:  Atom,
}

/// The different formats of a Client message's data,
/// as specified by ICCCM.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ClientMessageData {
    U8([u8; 20]),
    U16([u16; 10]),
    U32([u32; 5]),
}

impl ClientMessageData {
    /// Is the data in U8 format?
    pub(crate) fn is_u8(&self) -> bool {
        matches!(self, Self::U8(_))
    }

    /// Is the data in U16 format?
    pub(crate) fn is_u16(&self) -> bool {
        matches!(self, Self::U16(_))
    }

    /// Is the data in U32 format?
    pub(crate) fn is_u32(&self) -> bool {
        matches!(self, Self::U32(_))
    }
}

use std::convert::TryFrom;

macro_rules! _impl_tryfrom {
    ($t:ty, $count:expr, $variant:expr) => {
        impl TryFrom<&[$t]> for ClientMessageData {
            type Error = std::array::TryFromSliceError;

            fn try_from(data: &[$t]) -> Result<Self, Self::Error> {
                Ok($variant(<[$t; $count]>::try_from(data)?))
            }
        }
    };
}

_impl_tryfrom!(u8, 20, Self::U8);
_impl_tryfrom!(u16, 10, Self::U16);
_impl_tryfrom!(u32, 5, Self::U32);

// ============================== Unused ==============================
// ====================================================================

/// Window focus event
#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum FocusEvent {
    /// Window came into focus
    Gain,
    /// Window lost focus
    Lose,
}

/// A display event
#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum DisplayEvent {
    /// Area of a [`Window`] needs to be updated
    Expose(Rectangle),
    /// Window focus changed
    Focus(FocusEvent),
    /// Window dimensions changed
    Resize(Dimension),
}
