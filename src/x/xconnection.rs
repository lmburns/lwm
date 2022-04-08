//! The window manager

use crate::{
    config::{Config, GlobalSettings},
    core::{
        Atom,
        Button,
        MotionRecorder,
        Pid,
        Window,
        WindowClass,
        WindowMap,
        WindowState,
        WindowType,
        Xid,
        META_WINDOW_IC,
        MISSING_VALUE,
        TITLEBAR_HEIGHT,
    },
    error::Error,
    geometry::{Dimension, Extents, Point, Ratio, Rectangle, Strut},
    lwm_fatal,
    monitor::client::IcccmProps,
    x::{
        property::{Hints, IcccmWindowState, SizeHints},
        stream::Aux,
        utils::Stack,
    },
    WM_NAME,
};
use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use nix::poll::{poll, PollFd, PollFlags};
use once_cell::sync::Lazy;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::HashMap,
    convert::TryFrom,
    fs,
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::{
        io::AsRawFd,
        net::{UnixListener, UnixStream},
    },
    process,
    str::{self, FromStr},
    sync::Arc,
};
use strum::VariantNames;
use strum_macros::Display;
use tern::t;
use x11rb::{
    atom_manager,
    connection::{Connection, RequestConnection},
    cookie::Cookie,
    cursor::Handle as CursorHandle,
    errors::{ConnectionError, ReplyError},
    properties::{self, WmClass},
    protocol::{
        self,
        randr::{
            self,
            ConnectionExt as _,
            GetScreenResourcesReply,
            GetScreenSizeRangeReply,
            ListOutputPropertiesReply,
        },
        xkb::{self, ConnectionExt as _},
        xproto::{
            self,
            AtomEnum,
            ButtonIndex,
            ChangeGCAux,
            ChangeWindowAttributesAux,
            ClientMessageEvent,
            ConfigureWindowAux,
            ConnectionExt,
            CreateGCAux,
            CreateWindowAux,
            EventMask,
            GetAtomNameReply,
            GetGeometryReply,
            GetInputFocusReply,
            GetPropertyReply,
            GetSelectionOwnerReply,
            GetWindowAttributesReply,
            GrabMode,
            InputFocus,
            InternAtomReply,
            MapState,
            ModMask,
            PropMode,
            QueryPointerReply,
            QueryTreeReply,
            SetMode,
            WindowClass as XWindowClass,
            CLIENT_MESSAGE_EVENT,
        },
        ErrorKind,
        Event,
    },
    resource_manager::Database,
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

// === Atoms === [[[

/// An [`Atom`] is a unique ID corresponding to a string name that is used to
/// identify properties, types, and selections. See the [Client Properties][1]
/// and [Extended Properties][2] for more information, as well as [Window
/// Types][3], [Window Properties][4]
///
/// [1]: https://specifications.freedesktop.org/wm-spec/wm-spec-latest.html#idm45381393900464
/// [2]: https://tronche.com/gui/x/icccm/sec-4.html#s-4.1.2
/// [3]: https://specifications.freedesktop.org/wm-spec/latest/ar01s05.html#idm139870830002400
/// [4]: http://standards.freedesktop.org/wm-spec/latest/ar01s05.html#idm139870829988448
atom_manager! {
    pub(crate) Atoms: AtomsCookie {
        Any,
        // An X11-Atom
        ATOM,
        // A cardinal number
        CARDINAL,
        // An X11 window ID
        WINDOW,
        // A string
        STRING,
        // UTF-8 encoded string data
        UTF8_STRING,

        // ============ ICCCM client properties ============ [[[
        // Title or name of the window
        WM_NAME,
        // Consecutive null-term strings; Instance and class names
        WM_CLASS,
        // ID of another top-level window. Pop-up on behalf of window
        WM_TRANSIENT_FOR,
        // Forms name of machine running the client
        WM_CLIENT_MACHINE,
        // List of atoms identifying protocol between client and window
        WM_PROTOCOLS,
        // Indicate size, position, and perferences; Type is WM_SIZE_HINTS
        WM_NORMAL_HINTS,
        // Has atom if prompt of deletion or deletion is about to happen
        WM_DELETE_WINDOW,
        WM_WINDOW_ROLE,
        WM_CLIENT_LEADER,
        // Window may receieve a `ClientMessage` event
        WM_TAKE_FOCUS, // ]]]

        // ========== ICCCM window manager properties ====== [[[
        // Top-level windows not in withdrawn have this tag
        WM_STATE,
        // If wishes to place constraints on sizes of icon pixmaps
        WM_ICON_SIZE, // ]]]

        // ============== EWMH root properties ============= [[[
        // See: http://standards.freedesktop.org/wm-spec/latest/ar01s03.html
        //
        // Indicates which hints are supported
        _NET_SUPPORTED,
        // Set on root window to be the ID of a child window to indicate WM is active
        _NET_SUPPORTING_WM_CHECK,
        // All windows managed by the window manager with an
        // initial mapping order, starting with the oldest window
        _NET_CLIENT_LIST,
        // Array of null-terminated strings for all virtual desktops
        _NET_DESKTOP_NAMES,
        // Array of pairs of cardinals define top-left corner of each desktop viewport
        _NET_DESKTOP_VIEWPORT,
        // Indicate number of virtual desktops
        _NET_NUMBER_OF_DESKTOPS,
        // Window ID of active window or none if no window is focused
        _NET_ACTIVE_WINDOW,

        // == no
        // All windows managed by the window manager with a botom-to-top stacking order
        _NET_CLIENT_LIST_STACKING,
        // Array of 2 cardinals defining common size of desktops
        _NET_DESKTOP_GEOMETRY,
        // Index of current desktop
        _NET_CURRENT_DESKTOP,
        // Contains geometry for each desktop
        _NET_WORKAREA,
        // List of IDs for windows acting as virtual roots
        _NET_VIRTUAL_ROOTS,
        _NET_DESKTOP_LAYOUT,
        // Set to 1 when windows are hidden and desktop is shown
        _NET_SHOWING_DESKTOP, // ]]]

        // ============== EWMH root messages =============== [[[
        // Wanting to close a window muse send this request
        _NET_CLOSE_WINDOW,

        // no
        _NET_MOVERESIZE_WINDOW,
        _NET_WM_MOVERESIZE,
        _NET_REQUEST_FRAME_EXTENTS, // ]]]

        // ========== EWMH application properties ========== [[[
        // See: http://standards.freedesktop.org/wm-spec/latest/ar01s05.html
        _NET_WM_STRUT_PARTIAL,
        _NET_WM_DESKTOP,
        _NET_WM_STATE,
        _NET_WM_WINDOW_TYPE,

        // no
        // https://specifications.freedesktop.org/wm-spec/1.3/ar01s05.html

        // If set, preferred to WM_NAME
        _NET_WM_NAME,
        // If window manager displays name other than _NET_WM_NAME
        _NET_WM_VISIBLE_NAME,
        // Title of the icon (preferred over WM_ICON_NAME)
        _NET_WM_ICON_NAME,
        // If WM display an icon other athan _NET_WM_ICON_NAME
        _NET_WM_VISIBLE_ICON_NAME,
        _NET_WM_ALLOWED_ACTIONS,
        _NET_WM_STRUT,
        _NET_WM_ICON_GEOMETRY,
        _NET_WM_ICON,
        _NET_WM_PID,
        _NET_WM_HANDLED_ICONS,
        _NET_WM_USER_TIME,
        _NET_WM_USER_TIME_WINDOW,
        _NET_FRAME_EXTENTS,
        _NET_WM_OPAQUE_REGION,
        _NET_WM_BYPASS_COMPOSITOR,

        // === EWMH window states ===
        _NET_WM_STATE_HIDDEN,
        _NET_WM_STATE_FULLSCREEN,
        _NET_WM_STATE_BELOW,
        _NET_WM_STATE_ABOVE,
        _NET_WM_STATE_STICKY,
        _NET_WM_STATE_DEMANDS_ATTENTION,

        // no
        _NET_WM_STATE_MODAL,
        _NET_WM_STATE_MAXIMIZED_VERT,
        _NET_WM_STATE_MAXIMIZED_HORZ,
        _NET_WM_STATE_SHADED,
        _NET_WM_STATE_SKIP_TASKBAR,
        _NET_WM_STATE_SKIP_PAGER,
        _NET_WM_STATE_FOCUSED, // ]]]

        // =============== EWMH window types =============== [[[
        _NET_WM_WINDOW_TYPE_DOCK,
        _NET_WM_WINDOW_TYPE_DESKTOP,
        _NET_WM_WINDOW_TYPE_NOTIFICATION,
        _NET_WM_WINDOW_TYPE_DIALOG,
        _NET_WM_WINDOW_TYPE_UTILITY,
        _NET_WM_WINDOW_TYPE_TOOLBAR,

        // no
        _NET_WM_WINDOW_TYPE_MENU,
        _NET_WM_WINDOW_TYPE_SPLASH,
        _NET_WM_WINDOW_TYPE_DROPDOWN_MENU,
        _NET_WM_WINDOW_TYPE_POPUP_MENU,
        _NET_WM_WINDOW_TYPE_TOOLTIP,
        _NET_WM_WINDOW_TYPE_COMBO,
        _NET_WM_WINDOW_TYPE_DND,
        _NET_WM_WINDOW_TYPE_NORMAL, // ]]]

        // ================= EWMH protocols ================ [[[
        _NET_WM_PING,
        _NET_WM_SYNC_REQUEST,
        _NET_WM_FULLSCREEN_MONITORS, // ]]]

        // ============= System tray protocols ============= [[[
        _NET_SYSTEM_TRAY_ORIENTATION,
        _NET_SYSTEM_TRAY_OPCODE,
        _NET_SYSTEM_TRAY_ORIENTATION_HORZ,
        _NET_SYSTEM_TRAY_S0,
        _XEMBED,
        _XEMBED_INFO, // ]]]
    }
}

// impl ToString for Atoms {}
// ]]] === Atoms ===

// ============================ XConnection =========================== [[[]

/// Hold all current masks
struct Masks {
    /// Mask for root window events
    bspwm_root_event_mask: EventMask,
    /// Mask for client events
    client_event_mask:     EventMask,
    /// Mask for root window events
    root_event_mask:       EventMask,
    /// Mask for normal window events
    window_event_mask:     EventMask,
    /// Mask for mouse events
    frame_event_mask:      EventMask,
    /// Mask for mouse events
    mouse_event_mask:      EventMask,
    /// Mask for regrabbing events
    regrab_event_mask:     EventMask,
}

impl Masks {
    /// Create a new [`Masks`]
    fn new() -> Self {
        Self {
            bspwm_root_event_mask: EventMask::SUBSTRUCTURE_REDIRECT
                | EventMask::SUBSTRUCTURE_NOTIFY
                | EventMask::STRUCTURE_NOTIFY
                | EventMask::BUTTON_PRESS,
            client_event_mask:     EventMask::PROPERTY_CHANGE | EventMask::FOCUS_CHANGE,
            root_event_mask:       EventMask::PROPERTY_CHANGE
                | EventMask::SUBSTRUCTURE_REDIRECT
                | EventMask::STRUCTURE_NOTIFY
                | EventMask::BUTTON_PRESS
                | EventMask::POINTER_MOTION
                | EventMask::FOCUS_CHANGE,
            window_event_mask:     EventMask::PROPERTY_CHANGE
                | EventMask::STRUCTURE_NOTIFY
                | EventMask::FOCUS_CHANGE,
            frame_event_mask:      EventMask::STRUCTURE_NOTIFY
                | EventMask::SUBSTRUCTURE_REDIRECT
                | EventMask::SUBSTRUCTURE_NOTIFY
                | EventMask::BUTTON_PRESS
                | EventMask::BUTTON_RELEASE
                | EventMask::POINTER_MOTION,
            mouse_event_mask:      EventMask::BUTTON_PRESS
                | EventMask::BUTTON_RELEASE
                | EventMask::BUTTON_MOTION,
            regrab_event_mask:     EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE,
        }
    }
}

// black_gc: Gcontext,
// windows: Vec<WindowState>,
// pending_expose: HashSet<Window>,
// wm_protocols: Atom,
// wm_delete_window: Atom,
// sequences_to_ignore: BinaryHeap<Reverse<u16>>,
// drag_window: Option<(Window, (i16, i16))>,

/// The main connection to the X-Server
pub(crate) struct XConnection {
    /// Connections to the X-Server
    conn:            Aux,
    /// A hash mapping an [`Atom`] to a [`WindowType`]
    win_types:       HashMap<Atom, WindowType>,
    /// A hash mapping an [`Atom`] to a [`WindowState`]
    win_states:      HashMap<Atom, WindowState>,
    /// Configuration of the user
    config:          GlobalSettings,
    /// Background graphics context
    gctx:            xproto::Gcontext,
    /// Hold all current masks
    masks:           Masks,
    /// TODO: Define
    motion_recorder: MotionRecorder,
}

impl XConnection {
    /// Create a new [`XConnection`]
    pub(crate) fn new(conn: RustConnection, screen_num: usize, config: &Config) -> Result<Self> {
        log::trace!("creating a new `XConnection`");
        log::trace!("{:#?}", config);

        Self::check_extensions(&conn).context("failed to query extensions")?;
        let root = conn.setup().roots[screen_num].root;

        // Allocate a graphics context
        let gctx = conn.generate_id().context("failed to generate an `ID`")?;
        conn.create_gc(gctx, root, &CreateGCAux::new())?
            .check()
            .context("create graphics context")?;

        // conn.grab_server()
        //     .context("failed to grab server")?
        //     .check()
        //     .context("failed to check after grabbing server")?;

        let motion_id = conn.generate_id().context("failed to generate an `ID`")?;
        let aux = Aux::new(conn, screen_num).context("failed to create `Aux`")?;

        let mut xconn = Self {
            win_types: WindowType::to_hashmap(aux.get_atoms()),
            win_states: WindowState::to_hashmap(aux.get_atoms()),
            conn: aux,
            config: config.global.clone(),
            masks: Masks::new(),
            gctx,
            motion_recorder: MotionRecorder::new(motion_id),
        };

        xconn.init()?;
        // xconn.become_wm()?;

        // xconn
        //     .conn
        //     .ungrab_server()
        //     .context("failed to ungrab server")?
        //     .check()
        //     .context("failed to check after ungrabbing server")?;

        Ok(xconn)
    }

    // ========================== TESTING ==========================
    // ========================== TESTING ==========================
    /// testing func
    #[allow(clippy::unnecessary_wraps)]
    pub(crate) fn get_test(&self) -> Result<()> {
        log::debug!("requesting a `GetScreenSizeRangeReply` reply");

        let reply = self
            .aux()
            .get_property(
                false,
                self.root(),
                self.atoms()._NET_NUMBER_OF_DESKTOPS,
                AtomEnum::CARDINAL,
                0,
                u32::MAX,
            )?
            .reply()?;

        let num = reply
            .value32()
            .and_then(|mut x| x.next())
            .ok_or_else(|| Error::InvalidProperty("_NET_NUMBER_OF_DESKTOPS".to_owned()))?;

        println!("DESKTOP: {:#?}", num);

        Ok(())
    }

    // ========================== TESTING ==========================
    // ========================== TESTING ==========================

    // ========================= Accessor ========================= [[[

    /// Return the connection to the X-Server
    pub(crate) fn aux(&self) -> &RustConnection {
        self.conn.get_dpy()
    }

    /// Return the `root` window
    pub(crate) fn root(&self) -> xproto::Window {
        self.aux().setup().roots[self.screen()].root
    }

    /// Return the `root` window
    pub(crate) const fn atoms(&self) -> Atoms {
        *self.conn.get_atoms()
    }

    /// Return the focused screen number
    pub(crate) const fn screen(&self) -> usize {
        *self.conn.get_screen()
    }

    /// Return the meta window
    pub(crate) const fn meta_window(&self) -> u32 {
        *self.conn.get_meta_window()
    }

    // ]]] === Accessor ===

    // ======================== Initialize ======================== [[[

    /// Initialize the window manager
    fn init(&self) -> Result<()> {
        self.init_metawindow()?;
        self.aux()
            .configure_window(
                self.meta_window(),
                &ConfigureWindowAux::default().stack_mode(xproto::StackMode::BELOW),
            )?
            .check()
            .context("failed to configure meta window")?;

        self.aux()
            .randr_select_input(
                self.meta_window(),
                randr::NotifyMask::OUTPUT_CHANGE
                    | randr::NotifyMask::CRTC_CHANGE
                    | randr::NotifyMask::SCREEN_CHANGE,
            )
            .context("failed to select randr input")?
            .check()
            .context("failed to check randr::select_input")?;

        self.init_cursor();

        let aux = CreateWindowAux::new()
            .event_mask(EventMask::SUBSTRUCTURE_NOTIFY | EventMask::POINTER_MOTION);

        self.aux()
            .create_window(
                x11rb::COPY_DEPTH_FROM_PARENT, // depth
                self.motion_recorder.id,       // wid
                self.root(),                   // parent
                0,                             // x
                0,                             // y
                1,                             // width
                1,                             // height
                0,                             // border_width
                XWindowClass::INPUT_OUTPUT,    // class
                0,                             // visual
                &aux,
            )
            .context(format!(
                "failed to create motion recorder Window({:#0x})",
                self.motion_recorder.id
            ))?
            .check()
            .context("failed to check creating motion recorder window")?;

        Ok(())
    }

    /// Initialize the meta window
    pub(crate) fn init_metawindow(&self) -> Result<()> {
        log::debug!("creating `meta_window`");
        self.aux().create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            self.meta_window(),
            self.root(),
            -1,
            -1,
            1,
            1,
            0,
            xproto::WindowClass::INPUT_ONLY,
            x11rb::COPY_FROM_PARENT,
            &xproto::CreateWindowAux::default().override_redirect(1),
        )?;

        // self.grab_server()?;
        self.map_window(self.meta_window());
        // self.ungrab_server()?;

        Ok(())
    }

    /// Initializes the database and set the cursor
    fn init_cursor(&self) {
        log::debug!("initializing the Cursor to `left_ptr`");
        if let Ok(ref db) = Database::new_from_default(self.aux()).context("failed to get database") {
            CursorHandle::new(self.aux(), self.screen(), db).map(|cookie| {
                cookie.reply().map(|inner| {
                    let aux = ChangeWindowAttributesAux::default()
                        .cursor(inner.load_cursor(self.aux(), "left_ptr").ok());

                    self.change_window_attributes(&aux);
                })
            });
        }
    }

    /// Initalize a new [`Window`]
    pub(crate) fn init_window(&self, window: Window, focus_follows_mouse: bool) -> Result<()> {
        log::debug!(
            "initializing Window({:#0x}); focus_follows_mouse: {}",
            window,
            focus_follows_mouse
        );

        self.aux()
            .change_window_attributes(
                window,
                &ChangeWindowAttributesAux::default().event_mask(self.masks.window_event_mask),
            )
            .context(format!(
                "failed to `change_window_attributes` Window({:#0x})",
                window
            ))?
            .check()
            .context("failed to check changing window attributes")?;

        Ok(())
    }

    /// Initalize a window frame
    pub(crate) fn init_frame(&self, window: Window, focus_follows_mouse: bool) -> Result<()> {
        log::debug!(
            "initializing Frame({:#0x}); focus_follows_mouse: {}",
            window,
            focus_follows_mouse
        );

        let evmask = self.masks.frame_event_mask
            | (t!(focus_follows_mouse?(EventMask::ENTER_WINDOW): (EventMask::NO_EVENT)));

        self.aux()
            .change_window_attributes(
                window,
                &ChangeWindowAttributesAux::default().event_mask(evmask),
            )
            .context(format!(
                "failed to `change_window_attributes` Window({:#0x}) with mask({:?})",
                window, evmask
            ))?
            .check()
            .context("failed to check changing window attributes")?;

        Ok(())
    }

    /// Initialize an unmanaged [`Window`]
    pub(crate) fn init_unmanaged(&self, window: Window) -> Result<()> {
        log::debug!("initializing unmanaged Window({:#0x})", window);
        self.aux()
            .change_window_attributes(
                window,
                &ChangeWindowAttributesAux::default().event_mask(EventMask::STRUCTURE_NOTIFY),
            )
            .context(format!(
                "failed to `change_window_attributes` Window({:#0x})",
                window
            ))?
            .check()
            .context("failed to check changing window attributes")?;

        Ok(())
    }

    /// Initializes the wanted window manager properties
    fn init_properties<S: AsRef<str>>(&self, wm_name: &str, desktop_names: &[S]) -> Result<()> {
        log::debug!("initializing window manager properties");
        // Specifies instance and class names, separated by null
        let wm_class = META_WINDOW_IC;

        self.aux()
            .change_property8(
                PropMode::REPLACE,
                self.meta_window(),
                self.atoms()._NET_WM_NAME,
                self.atoms().UTF8_STRING,
                wm_name.as_bytes(),
            )
            .context("failed to replace `_NET_WM_NAME`")?
            .check()
            .context("failed to check replacing `_NET_WM_NAME`")?;

        // set_icccm_window_class
        self.aux()
            .change_property8(
                PropMode::REPLACE,
                self.meta_window(),
                self.atoms().WM_CLASS,
                self.atoms().UTF8_STRING,
                wm_class.as_bytes(),
            )
            .context("failed to replace `WM_CLASS`")?
            .check()
            .context("failed to check replacing `WM_CLASS`")?;

        self.aux()
            .change_property32(
                PropMode::REPLACE,
                self.meta_window(),
                self.atoms()._NET_WM_PID,
                self.atoms().CARDINAL,
                &[process::id()],
            )
            .context("failed to replace `_NET_WM_PID`")?
            .check()
            .context("failed to check replacing `_NET_WM_PID`")?;

        self.aux()
            .change_property32(
                PropMode::REPLACE,
                self.root(),
                self.atoms()._NET_SUPPORTING_WM_CHECK,
                self.atoms().WINDOW,
                &[self.meta_window()],
            )
            .context("failed to replace `_NET_SUPPORTING_WM_CHECK`")?
            .check()
            .context("failed to check replacing `_NET_SUPPORTING_WM_CHECK`")?;

        self.aux()
            .change_property8(
                PropMode::REPLACE,
                self.root(),
                self.atoms()._NET_WM_NAME,
                self.atoms().UTF8_STRING,
                wm_name.as_bytes(),
            )
            .context("failed to replace `_NET_WM_NAME`")?
            .check()
            .context("failed to check replacing `_NET_WM_NAME`")?;

        self.aux()
            .change_property8(
                PropMode::REPLACE,
                self.root(),
                self.atoms().WM_CLASS,
                self.atoms().UTF8_STRING,
                wm_class.as_bytes(),
            )
            .context("failed to replace `WM_CLASS`")?
            .check()
            .context("failed to check replacing `WM_CLASS`")?;

        self.aux()
            .change_property32(
                PropMode::REPLACE,
                self.meta_window(),
                self.atoms()._NET_SUPPORTING_WM_CHECK,
                self.atoms().WINDOW,
                &[self.meta_window()],
            )
            .context("failed to replace `_NET_SUPPORTING_WM_CHECK`")?
            .check()
            .context("failed to check replacing `_NET_SUPPORTING_WM_CHECK`")?;

        self.init_supported()?;

        self.aux()
            .change_property32(
                PropMode::REPLACE,
                self.root(),
                self.atoms()._NET_WM_PID,
                self.atoms().CARDINAL,
                &[process::id()],
            )
            .context("failed to replace `_NET_WM_PID`")?
            .check()
            .context("failed to check replacing `_NET_WM_PID`")?;

        self.aux()
            .delete_property(self.root(), self.atoms()._NET_CLIENT_LIST)
            .context("failed to delete property `_NET_CLIENT_LIST`")?
            .check()
            .context("failed to check replacing `_NET_CLIENT_LIST`")?;

        self.update_desktops(desktop_names)?;

        Ok(())
    }

    // ]]] === Initialize ===

    // ====================== Window Manager ====================== [[[

    /// Make an attempt to become the window manager
    pub(crate) fn become_wm(&self) -> Result<()> {
        log::debug!("attempting to become the window manager");

        if let Err(ReplyError::X11Error(err)) = self
            .aux()
            .change_window_attributes(
                self.root(),
                &ChangeWindowAttributesAux::new()
                    .event_mask(EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY),
            )
            .context("another window manager is currently running")?
            .check()
        {
            if err.error_kind == ErrorKind::Access {
                return Err(anyhow!("another window manager is currently running"));
            }

            return Err(anyhow!("failed to setup LWM window manager"));
        }

        Ok(())
    }

    /// Send a [`ClientMessageEvent`]
    pub(crate) fn send_client_message(&self, window: Window, atom: Atom, type_: Atom) -> Result<()> {
        let data = [atom, x11rb::CURRENT_TIME, 0, 0, 0];
        let event = ClientMessageEvent::new(32, window, type_, data);
        log::debug!(
            "sending a `ClientMessage` for Window({:#0x}); atom: {}, type: {}",
            window,
            atom,
            type_
        );

        self.aux()
            .send_event(false, window, EventMask::NO_EVENT, &event)
            .context(format!(
                "failed to send event. Window: {:#0x}, Type: {}",
                event.window, event.type_
            ))?
            .check()
            .context(format!(
                "failed to check sending event. Window: {:#0x}, Type: {}",
                event.window, event.type_
            ))?;

        // Is this needed?
        self.flush();

        Ok(())
    }

    /// Send a [`ClientMessageEvent`] using `WM_PROTOCOLS`
    pub(crate) fn send_protocol_client_message(&self, window: Window, atom: Atom) -> Result<()> {
        self.send_client_message(window, atom, self.atoms().WM_PROTOCOLS)
    }

    /// Set an [`Atom`]s value (cardinal)
    pub(crate) fn set_atom(&self, window: Window, atom: Atom, value: &[u32]) -> Result<()> {
        log::debug!("changing `{}` in Window({:#0x})", atom, window);
        self.aux()
            .change_property32(
                PropMode::REPLACE,
                window,
                atom,
                self.atoms().CARDINAL,
                value,
            )
            .context(format!(
                "failed to set `{}` in Window({:#0x})",
                atom, window
            ))?
            .check()
            .context(format!("failed to check setting `{}`", atom));

        Ok(())
    }

    // ====================== Testing Values ====================== [[[

    /// Check whether the window supports any `WM_PROTOCOLS`
    pub(crate) fn window_supports_protocols(&self, window: Window, protocols: &[Atom]) -> bool {
        log::debug!(
            "checking if Window({:#0x}) supports protocols {:?}",
            window,
            protocols
        );
        self.aux()
            .get_property(
                false,
                window,
                self.atoms().WM_PROTOCOLS,
                self.atoms().ATOM, // AtomEnum::ATOM,
                0,
                u32::MAX,
            )
            .map_or(false, |cookie| {
                cookie.reply().map_or(false, |reply| {
                    reply.value32().map_or(false, |mut window_protocols| {
                        window_protocols.any(|protocol| protocols.contains(&protocol))
                    })
                })
            })
    }

    /// Check whether the window is in any of the given [`states`](Atom)
    pub(crate) fn window_is_any_of_states(&self, window: Window, states: &[Atom]) -> bool {
        log::debug!(
            "checking if Window({:#0x}) supports states {:?}",
            window,
            states
        );
        self.aux()
            .get_property(
                false,
                window,
                self.atoms()._NET_WM_STATE,
                self.atoms().ATOM, // AtomEnum::ATOM,
                0,
                u32::MAX,
            )
            .map_or(false, |cookie| {
                cookie.reply().map_or(false, |reply| {
                    reply.value32().map_or(false, |mut window_states| {
                        window_states.any(|state| states.contains(&state))
                    })
                })
            })
    }

    /// Check whether the window is any of the given [`types`](Atom)
    pub(crate) fn window_is_any_of_types(&self, window: Window, types: &[Atom]) -> bool {
        log::debug!(
            "checking if Window({:#0x}) supports types {:?}",
            window,
            types
        );
        self.aux()
            .get_property(
                false,
                window,
                self.atoms()._NET_WM_WINDOW_TYPE,
                self.atoms().ATOM, // AtomEnum::ATOM,
                0,
                u32::MAX,
            )
            .map_or(false, |cookie| {
                cookie.reply().map_or(false, |reply| {
                    reply.value32().map_or(false, |mut window_types| {
                        window_types.any(|type_| types.contains(&type_))
                    })
                })
            })
    }

    /// Should the window manager manage this [`Window`]?
    pub(crate) fn must_manage_window(&self, window: Window) -> bool {
        log::debug!("checking if Window({:#0x}) should be managed", window);
        let do_not_manage = self
            .aux()
            .get_window_attributes(window)
            .map_or(false, |cookie| {
                cookie.reply().map_or(false, |reply| {
                    reply.override_redirect || reply.class == xproto::WindowClass::INPUT_ONLY
                })
            });

        if do_not_manage {
            return false;
        }

        let to_exclude = &[
            self.atoms()._NET_WM_WINDOW_TYPE_DOCK,
            self.atoms()._NET_WM_WINDOW_TYPE_TOOLBAR,
        ];

        !self.window_is_any_of_types(window, to_exclude)
    }

    /// Determine if a window must be freed based on given its state, type, or
    /// geometry
    pub(crate) fn must_free_window(&self, window: Window) -> bool {
        log::debug!("checking if Window({:#0x}) should be freed", window);
        if self.get_window_desktop(window) == Some(0xFFFF_FFFF)
            || self.window_is_any_of_states(window, &[self.atoms()._NET_WM_STATE_MODAL])
            || self.window_is_any_of_types(window, &[
                self.atoms()._NET_WM_WINDOW_TYPE_DIALOG,
                self.atoms()._NET_WM_WINDOW_TYPE_UTILITY,
                self.atoms()._NET_WM_WINDOW_TYPE_TOOLBAR,
                self.atoms()._NET_WM_WINDOW_TYPE_SPLASH,
            ])
        {
            return true;
        }

        self.get_window_geometry(window).map_or(false, |geometry| {
            self.get_icccm_window_size_hints(window, None, &None)
                .1
                .map_or(false, |sh| {
                    match (sh.min_width, sh.min_height, sh.max_width, sh.max_height) {
                        (Some(miw), Some(mih), Some(maw), Some(mah)) =>
                            maw > 0 && mah > 0 && maw == miw && mah == mih,
                        _ => false,
                    }
                })
        })
    }

    /// Check if the given [`Window`] is mappable
    pub(crate) fn window_is_mappable(&self, window: Window) -> bool {
        log::debug!("checking if Window({:#0x}) is mappable", window);
        self.aux()
            .get_window_attributes(window)
            .map_or(false, |cookie| {
                cookie.reply().map_or(false, |reply| {
                    let default_state = properties::WmHintsState::Normal;
                    let initial_state = properties::WmHints::get(self.aux(), window).ok().map_or(
                        default_state,
                        |cookie| {
                            cookie.reply().map_or(default_state, |reply| {
                                reply.initial_state.map_or(default_state, |i| i)
                            })
                        },
                    );

                    reply.class != xproto::WindowClass::INPUT_ONLY
                        && !self.window_is_any_of_states(window, &[self.atoms()._NET_WM_STATE_HIDDEN])
                        && matches!(initial_state, properties::WmHintsState::Normal)
                })
            })
    }

    /// Test whether the window is in fullscreen using
    pub(crate) fn window_is_fullscreen(&self, window: Window) -> bool {
        log::debug!(
            "checking `_NET_WM_STATE_FULLSCREEN` for Window({:#0x})",
            window
        );
        self.window_is_any_of_states(window, &[self.atoms()._NET_WM_STATE_FULLSCREEN])
    }

    /// Test whether the window should be above other windows
    pub(crate) fn window_is_above(&self, window: Window) -> bool {
        log::debug!("checking `_NET_WM_STATE_ABOVE` for Window({:#0x})", window);
        self.window_is_any_of_states(window, &[self.atoms()._NET_WM_STATE_ABOVE])
    }

    /// Test whether the window should be below other windows
    pub(crate) fn window_is_below(&self, window: Window) -> bool {
        log::debug!("checking `_NET_WM_STATE_BELOW` for Window({:#0x})", window);
        self.window_is_any_of_states(window, &[self.atoms()._NET_WM_STATE_BELOW])
    }

    /// Test whether the window's position should remain fixed
    pub(crate) fn window_is_sticky(&self, window: Window) -> bool {
        log::debug!("checking `_NET_WM_STATE_STICKY` for Window({:#0x})", window);

        self.window_is_any_of_states(window, &[self.atoms()._NET_WM_STATE_STICKY])
            || self.get_window_desktop(window) == Some(0xFFFF_FFFF)
    }

    // ]]] ===== Testing Values =====

    // ===================== Window Information ==================== [[[

    /// Return windows managed by the window manager
    pub(crate) fn windows(&self, all: bool) -> Result<Vec<Window>> {
        let mut windows = vec![];
        if all {
            log::debug!("querying for all windows");
            let tree = self.query_tree(self.root())?;

            for win in tree.children {
                windows.push(win);
            }
        } else {
            log::debug!("querying for windows managed by the window manager");
            let reply = self
                .aux()
                .get_property(
                    false,
                    self.root(),
                    self.atoms()._NET_CLIENT_LIST,
                    AtomEnum::WINDOW,
                    0,
                    u32::MAX,
                )
                .context("failed to get property `_NET_CLIENT_LIST`")?
                .reply()
                .context("failed to get property `_NET_CLIENT_LIST` reply")?;

            for win in reply
                .value32()
                .ok_or_else(|| Error::InvalidProperty(String::from("_NET_CLIENT_LIST")))?
            {
                windows.push(win);
            }
        }
        Ok(windows)
    }

    /// Get the [`Window`]s attributes
    pub(crate) fn get_window_attrs(&self, window: Window) -> Result<(WindowClass, WindowMap)> {
        let attr = self.get_window_attributes(window)?;
        log::debug!(
            "WindowAttributes: id: {}, win_gravity: {:?}, bit_gravity: {:?}",
            window,
            attr.win_gravity,
            attr.bit_gravity
        );
        Ok((
            WindowClass::from_atoms(attr.class.into())?,
            WindowMap::from_atoms(attr.map_state.into())?,
        ))
    }

    /// Get the given [`Window`]s class name as a `String`
    pub(crate) fn get_window_class(&self, window: Window) -> Result<String> {
        log::debug!("getting `WM_CLASS` for Window({:#0x})", window);
        let reply = self
            .aux()
            .get_property(
                false,
                window,
                self.atoms().WM_CLASS,
                AtomEnum::STRING,
                0,
                u32::MAX,
            )
            .context("failed to get `WM_CLASS`")?
            .reply()
            .context("failed to get `WM_CLASS` reply")?;

        // Skip the first null terminated string and extract the second
        let iter = reply
            .value
            .into_iter()
            .skip_while(|x| *x != 0)
            .skip(1)
            .take_while(|x| *x != 0);

        // Extract the second null terminated string
        let class = str::from_utf8(&iter.collect::<Vec<_>>())?.to_owned();
        log::debug!("Window({:#0x}) is class {}", window, class);
        Ok(class)
    }

    /// Get the desktop the given [`Window`] is in
    pub(crate) fn get_window_desktop(&self, window: Window) -> Option<usize> {
        log::debug!("getting `_NET_WM_DESKTOP` for Window({:#0x})", window);
        self.aux()
            .get_property(
                false,
                window,
                self.atoms()._NET_WM_DESKTOP,
                AtomEnum::CARDINAL,
                0,
                u32::MAX,
            )
            .ok()?
            .reply()
            .map_or(None, |desktop| {
                let desktop: Vec<u32> = desktop.value32()?.collect();

                if desktop.is_empty() {
                    None
                } else {
                    Some(desktop[0] as usize)
                }
            })
    }

    // ]]] === Window Information ===

    // ========================= Actions ========================== [[[

    /// Create a window matching the given [`Rectangle`]
    pub(crate) fn create_frame(&self, rect: Rectangle) -> Result<Window> {
        log::debug!("creating a frame: Rectangle({})", rect);
        let wid = self.generate_id().context("failed to generate an ID")?;
        let aux = CreateWindowAux::new()
            .backing_store(Some(xproto::BackingStore::ALWAYS))
            .event_mask(EventMask::EXPOSURE | EventMask::KEY_PRESS);

        self.aux()
            .create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                wid,
                self.root(),
                rect.point.x as i16,
                rect.point.y as i16,
                rect.dimension.width as u16,
                rect.dimension.height as u16,
                0,
                xproto::WindowClass::INPUT_OUTPUT,
                0,
                &aux,
            )
            .context(format!("failed to create Window({:#0x})", wid))?
            .check()
            .context(format!("failed check creating Window({:#0x})", wid))?;

        self.flush();

        Ok(wid)
    }

    /// Create a new handle for the window manager
    ///
    /// This is a recreation of the `meta_window`
    pub(crate) fn create_handle(&self) -> Result<Window> {
        let wid = self.generate_id().context("failed to generate an ID")?;
        log::debug!("creating a `meta_window`: {:#0x}", wid);
        let aux = CreateWindowAux::new().override_redirect(1);

        self.aux()
            .create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                wid,
                self.root(),
                -2,
                -2,
                1,
                1,
                0,
                xproto::WindowClass::INPUT_ONLY,
                0,
                &aux,
            )
            .context(format!("failed to create Window({:#0x})", wid))?
            .check()
            .context(format!("failed check creating Window({:#0x})", wid))?;

        self.flush();

        Ok(wid)
    }

    /// Focus the given [`Window`]
    pub(crate) fn focus_window(&self, window: Window) -> Result<()> {
        log::debug!("focusing Window({:#0x})", window);
        self.aux()
            .set_input_focus(InputFocus::PARENT, window, x11rb::CURRENT_TIME)
            .context(format!(
                "failed to `set_input_focus` for Window({:#0x})",
                window
            ))?
            .check()
            .context(format!(
                "failed to check `set_input_focus` for Window({:#0x})",
                window
            ))?;

        self.aux()
            .change_property32(
                PropMode::REPLACE,
                self.root(),
                self.atoms()._NET_ACTIVE_WINDOW,
                AtomEnum::WINDOW,
                &[window],
            )
            .context("failed to replace property `_NET_ACTIVE_WINDOW`")?
            .check()
            .context("failed to replace property `_NET_ACTIVE_WINDOW`")?;

        Ok(())
    }

    /// Unfocus current window, moving the focus to the `meta_window`
    pub(crate) fn unfocus(&self) -> Result<()> {
        log::debug!("unfocusing `meta_window`");
        self.aux()
            .set_input_focus(InputFocus::PARENT, self.meta_window(), x11rb::CURRENT_TIME)
            .context("failed to unfocus `meta_window`")?
            .check()
            .context("failed to check unfocusing `meta_window`")?;

        self.delete_property(self.atoms()._NET_ACTIVE_WINDOW)?;

        Ok(())
    }

    // TODO:
    /// Set the current input focus
    pub(crate) fn clear_input_focus(&self) -> Result<()> {
        log::debug!("clearing `input_focus`");
        self.aux()
            .set_input_focus(InputFocus::POINTER_ROOT, self.root(), x11rb::CURRENT_TIME)
            .context("failed to clear `input_focus`")?
            .check()
            .context("failed to check setting `input_focus`")?;

        Ok(())
    }

    /// Modify the [`Window`]'s `border_width`
    ///
    /// Doesn't have to be from the user's configuration
    pub(crate) fn set_window_border_width(&self, window: Window, width: u32) -> Result<()> {
        log::debug!("setting Window({:#0x}) border width {}", window, width);
        self.aux()
            .configure_window(window, &ConfigureWindowAux::default().border_width(width))
            .context(format!(
                "failed to set Window({:#0x}) border width to {}",
                window, width
            ))?
            .check()
            .context(format!("failed to check setting border width to {}", width))?;

        Ok(())
    }

    /// Change the color of the [`Window`]'s border
    pub(crate) fn set_window_border_color(&self, window: Window, color: u32) -> Result<()> {
        log::debug!("setting Window({:#0x}) border color {}", window, color);
        self.aux()
            .change_window_attributes(
                window,
                &ChangeWindowAttributesAux::default().border_pixel(color),
            )
            .context(format!(
                "failed to set Window({:#0x}) border color to {}",
                window, color
            ))?
            .check()
            .context(format!("failed to check setting border color to {}", color))?;

        Ok(())
    }

    /// Change the color of the [`Window`]'s background
    pub(crate) fn set_window_background_color(&self, window: Window, color: u32) -> Result<()> {
        log::debug!("setting Window({:#0x}) bg color {}", window, color);
        if let Ok(r) = self.get_window_geometry(window) {
            self.aux()
                .change_gc(self.gctx, &ChangeGCAux::new().foreground(color))
                .context(format!(
                    "failed to change Window({:#0x}) foreground color",
                    window
                ))?
                .check()
                .context("failed to check changing foreground color")?;

            self.aux()
                .poly_fill_rectangle(window, self.gctx, &[xproto::Rectangle {
                    x:      0,
                    y:      0,
                    width:  r.dimension.width as u16,
                    height: r.dimension.height as u16,
                }])
                .context(format!(
                    "failed to fill rectangle in Window({:#0x})",
                    window
                ))?
                .check()
                .context("failed to check filling rectangle")?;
        }

        Ok(())
    }

    /// Modify the [`Window`]'s offset to the frame
    pub(crate) fn update_window_offset(&self, window: Window, frame: Window) -> Result<()> {
        log::debug!("updating Window({:#0x}) offset", window);
        if let Ok(frame) = self.get_window_geometry(frame) {
            if let Ok(geom) = self.get_window_geometry(window) {
                let event = xproto::ConfigureNotifyEvent {
                    response_type: xproto::CONFIGURE_NOTIFY_EVENT,
                    sequence: 0,
                    event: window,
                    window,
                    above_sibling: x11rb::NONE,
                    x: (frame.point.x + geom.point.x) as i16,
                    y: (frame.point.y + geom.point.y) as i16,
                    width: geom.dimension.width as u16,
                    height: geom.dimension.height as u16,
                    border_width: 0,
                    override_redirect: false,
                };

                self.aux()
                    .send_event(false, window, EventMask::STRUCTURE_NOTIFY, &event)
                    .context(format!("failed to update Window({:#0x})'s offset", window))?
                    .check()
                    .context(format!(
                        "failed to check updating Window({:#0x})'s offset",
                        window
                    ))?;
            }
        }

        Ok(())
    }

    // TODO: Finish
    /// Add a new window that should be managed by the WM
    pub(crate) fn manage_window(&mut self, window: Window, geom: &GetGeometryReply) -> Result<()> {
        log::debug!("managing Window({:#0x})", window);
        let screen = &self.aux().setup().roots[self.screen()];
        // assert!(self.find_window_by_id(win).is_none());

        let frame_win = self.aux().generate_id()?;
        let win_aux = CreateWindowAux::new()
            .event_mask(
                EventMask::EXPOSURE
                    | EventMask::SUBSTRUCTURE_NOTIFY
                    | EventMask::BUTTON_PRESS
                    | EventMask::BUTTON_RELEASE
                    | EventMask::POINTER_MOTION
                    | EventMask::ENTER_WINDOW,
            )
            .background_pixel(screen.white_pixel);

        self.aux().create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            frame_win,
            screen.root,
            geom.x,
            geom.y,
            geom.width,
            geom.height + TITLEBAR_HEIGHT,
            1,
            XWindowClass::INPUT_OUTPUT,
            0,
            &win_aux,
        )?;

        self.grab_server()?;
        self.aux().change_save_set(SetMode::INSERT, window)?;
        let cookie = self
            .aux()
            .reparent_window(window, frame_win, 0, TITLEBAR_HEIGHT as _)?;
        self.map_window(window)?;
        self.map_window(frame_win)?;
        self.ungrab_server()?;

        // TODO: Down

        // self.windows.push(WindowState::new(win, frame_win, geom));

        // Ignore all events caused by reparent_window(). All those events have the
        // sequence number of the reparent_window() request, thus remember its
        // sequence number. The grab_server()/ungrab_server() is done so that
        // the server does not handle other clients in-between, which could
        // cause other events to get the same sequence number.

        // self.sequences_to_ignore
        //     .push(Reverse(cookie.sequence_number() as u16));
        Ok(())
    }

    /// Resize a [`Window`]
    pub(crate) fn resize_window(&self, window: Window, dim: Dimension) -> Result<()> {
        log::debug!("resizing Window({:#0x}): ({})", window, dim);
        self.aux()
            .configure_window(window, &dim.to_aux())
            .context(format!("failed to resize Window({:#0x})", window))?
            .check()
            .context(format!("failed to check resizing Window({:#0x})", window))?;

        Ok(())
    }

    /// Place a [`Window`] on the screen
    pub(crate) fn place_window(&self, window: Window, rect: Rectangle) -> Result<()> {
        log::debug!("placing Window({:#0x}): {}", window, rect);
        self.aux()
            .configure_window(window, &rect.to_aux(self.config.border_width))
            .context(format!("failed to resize Window({:#0x})", window))?
            .check()
            .context(format!("failed to check resizing Window({:#0x})", window))?;

        Ok(())
    }

    // ========================== Update ========================== [[[

    /// Update the [`Window`]s managed by the window-manager
    pub(crate) fn update_client_list(&self, clients: &[Window]) -> Result<()> {
        log::debug!("updating `_NET_CLIENT_LIST`: {:?}", clients);
        self.aux()
            .change_property32(
                PropMode::REPLACE,
                self.root(),
                self.atoms()._NET_CLIENT_LIST,
                self.atoms().WINDOW,
                clients,
            )
            .context("failed to update `_NET_CLIENT_LIST`")?
            .check()
            .context("failed to check updating `_NET_CLIENT_LIST`")?;

        Ok(())
    }

    /// Update the [`Window`]s in `_NET_CLIENT_LIST_STACKING`
    pub(crate) fn update_client_list_stacking(&self, clients: &[Window]) -> Result<()> {
        log::debug!("updating `_NET_CLIENT_LIST_STACKING`: {:?}", clients);
        self.aux()
            .change_property32(
                PropMode::REPLACE,
                self.root(),
                self.atoms()._NET_CLIENT_LIST_STACKING,
                self.atoms().WINDOW,
                clients,
            )
            .context("failed to replace `_NET_CLIENT_LIST_STACKING`")?
            .check()
            .context("failed to check replacing `_NET_CLIENT_LIST_STACKING`")?;

        Ok(())
    }

    /// Change the name of the desktops
    pub(crate) fn update_desktops<S: AsRef<str>>(&self, desktop_names: &[S]) -> Result<()> {
        log::debug!(
            "updating `_NET_NUMBER_OF_DESKTOPS`: [{:?}]",
            desktop_names.iter().map(AsRef::as_ref).join(", ")
        );
        self.aux()
            .change_property32(
                PropMode::REPLACE,
                self.root(),
                self.atoms()._NET_NUMBER_OF_DESKTOPS,
                self.atoms().CARDINAL,
                &[desktop_names.len() as u32],
            )
            .context("failed to replace `_NET_NUMBER_OF_DESKTOPS`")?
            .check()
            .context("failed to check replacing `_NET_NUMBER_OF_DESKTOPS`")?;

        log::debug!("updating `_NET_DESKTOP_NAMES`");
        self.aux()
            .change_property8(
                PropMode::REPLACE,
                self.root(),
                self.atoms()._NET_DESKTOP_NAMES,
                self.atoms().UTF8_STRING,
                format!("{}\0", desktop_names.iter().map(AsRef::as_ref).join("\0")).as_bytes(),
            )
            .context("failed to replace `_NET_DESKTOP_NAMES`")?
            .check()
            .context("failed to check replacing `_NET_DESKTOP_NAMES`")?;

        Ok(())
    }

    // fn set_window_above(&self, window: Window, on: bool) {}
    // fn set_window_below(&self, window: Window, on: bool) {}

    /// Toggle full screen mode
    #[allow(clippy::unused_self)]
    pub(crate) fn toggle_fullscreen(&self, window: Window, on: bool) {
        // use self.aux.screen_width
        todo!()
    }

    // ]]] === Update ===

    // ]]] === Actions ===

    // ======================= Window State ======================= [[[

    /// Get a states of all [`Window`]s
    pub(crate) fn get_window_states(&self, window: Window) -> Vec<WindowState> {
        log::debug!("querying for Window({:#0x})'s state", window);
        let mut wm_states = vec![];

        // for state in reply
        //     .value32()
        //     .ok_or_else(|| Error::InvalidProperty(String::from("_NET_WM_STATE")))?
        // {
        //     let state = WindowState::from_atoms(&self.atoms(), state)?;
        //     log::debug!("WindowState: id: {}, state: {}", win, state);
        //     states.push(state);
        // }

        if let Some(reply) = self
            .aux()
            .get_property(
                false,
                window,
                self.atoms()._NET_WM_STATE,
                AtomEnum::ATOM,
                0,
                u32::MAX,
            )
            .ok()
            .and_then(|cookie| cookie.reply().ok())
        {
            if let Some(states) = reply.value32().map::<Vec<u32>, _>(Iterator::collect) {
                for state in states {
                    if let Some(state) = self.get_window_state_from_atom(state) {
                        wm_states.push(state);
                    }
                }
            }
        }

        wm_states
    }

    /// Get the first state of the given [`Window`]
    pub(crate) fn get_window_preferred_state(&self, window: Window) -> Option<WindowState> {
        log::debug!("getting Window({:#0x})'s preferred state", window);
        self.get_window_states(window).get(0).copied()
    }

    /// Set the `_NET_WM_STATE` property
    pub(crate) fn set_window_state_atom(
        &self,
        window: Window,
        state_atom: Atom,
        on: bool,
    ) -> Result<()> {
        if on {
            log::debug!("setting `_NET_WM_STATE`: {}", state_atom);
            if self.window_is_any_of_states(window, &[state_atom]) {
                return Ok(());
            }

            self.aux()
                .change_property32(
                    PropMode::APPEND,
                    window,
                    self.atoms()._NET_WM_STATE,
                    AtomEnum::ATOM,
                    &[state_atom],
                )
                .context("failed to append property `_NET_WM_STATE`")?
                .check()
                .context("failed to append property `_NET_WM_STATE`")?;

            Ok(())
        } else {
            let mut states = self
                .aux()
                .get_property(
                    false,
                    window,
                    self.atoms()._NET_WM_STATE,
                    self.atoms().ATOM,
                    0,
                    u32::MAX,
                )
                .map_or(vec![], |cookie| {
                    cookie.reply().map_or(vec![], |reply| {
                        reply.value32().map_or(vec![], |window_states| {
                            let mut states = Vec::with_capacity(reply.value_len as usize);
                            window_states.for_each(|state| states.push(state));
                            states
                        })
                    })
                });

            states.retain(|&state| state != state_atom);
            log::debug!("setting `_NET_WM_STATE`: {:?}", states);

            self.aux()
                .change_property32(
                    PropMode::REPLACE,
                    window,
                    self.atoms()._NET_WM_STATE,
                    AtomEnum::ATOM,
                    &states,
                )
                .context("failed to replace property `_NET_WM_STATE`")?
                .check()
                .context("failed to replace property `_NET_WM_STATE`")?;

            Ok(())
        }
    }

    /// Set the [`WindowState`] of the given [`Window`]
    pub(crate) fn set_window_state(
        &self,
        window: Window,
        state: WindowState,
        on: bool,
    ) -> Result<()> {
        log::debug!("setting Window({:#0x})'s state: {:?}", window, state);
        self.set_window_state_atom(
            window,
            match state {
                WindowState::Modal => self.atoms()._NET_WM_STATE_MODAL,
                WindowState::Sticky => self.atoms()._NET_WM_STATE_STICKY,
                WindowState::MaximizedVert => self.atoms()._NET_WM_STATE_MAXIMIZED_VERT,
                WindowState::MaximizedHorz => self.atoms()._NET_WM_STATE_MAXIMIZED_HORZ,
                WindowState::Shaded => self.atoms()._NET_WM_STATE_SHADED,
                WindowState::SkipTaskbar => self.atoms()._NET_WM_STATE_SKIP_TASKBAR,
                WindowState::SkipPager => self.atoms()._NET_WM_STATE_SKIP_PAGER,
                WindowState::Hidden => self.atoms()._NET_WM_STATE_HIDDEN,
                WindowState::Fullscreen => self.atoms()._NET_WM_STATE_FULLSCREEN,
                WindowState::Above => self.atoms()._NET_WM_STATE_ABOVE,
                WindowState::Below => self.atoms()._NET_WM_STATE_BELOW,
                WindowState::DemandsAttention => self.atoms()._NET_WM_STATE_DEMANDS_ATTENTION,
            },
            on,
        )
        .context(format!("failed to set window state: {}", state))?;

        Ok(())
    }

    /// Set a window's `WM_STATE` property
    pub(crate) fn set_icccm_window_state(
        &self,
        window: Window,
        state: IcccmWindowState,
    ) -> Result<()> {
        log::debug!("setting ICCCM Window({:#0x})'s state: {:?}", window, state);
        self.aux()
            .change_property32(
                PropMode::REPLACE,
                window,
                self.atoms().WM_STATE,
                self.atoms().CARDINAL,
                &[state.into(), 0],
            )
            .context("failed to set `IcccmWindowState`")?
            .check()
            .context("failed to check setting `IcccmWindowState`")?;

        Ok(())
    }

    /// Get an `icccm` window's `WM_STATE` property
    pub(crate) fn get_icccm_window_class(&self, window: Window) -> String {
        log::debug!(
            "requesting ICCCM Window({:#0x})'s `WM_STATE` property",
            window
        );
        WmClass::get(self.aux(), window).map_or(String::from(MISSING_VALUE), |cookie| {
            cookie.reply().map_or(String::from(MISSING_VALUE), |reply| {
                str::from_utf8(reply.class()).map_or(String::from(MISSING_VALUE), String::from)
            })
        })
    }

    /// Get an `icccm` window's `WM_NAME` property
    pub(crate) fn get_icccm_window_name(&self, window: Window) -> String {
        log::debug!("requesting Window({:#0x})'s `WM_NAME` property", window);
        self.aux()
            .get_property(
                false,
                window,
                self.atoms().WM_NAME,
                self.atoms().UTF8_STRING,
                0,
                u32::MAX,
            )
            .map_or(String::from(MISSING_VALUE), |cookie| {
                cookie.reply().map_or(String::from(MISSING_VALUE), |reply| {
                    str::from_utf8(&reply.value8().map_or(vec![], Iterator::collect))
                        .map_or(String::from(MISSING_VALUE), ToOwned::to_owned)
                })
            })
    }

    /// Get an `icccm` window's name contained in the `WM_CLASS` property
    pub(crate) fn get_icccm_window_instance(&self, window: Window) -> String {
        log::debug!(
            "requesting ICCCM Window({:#0x})'s `WM_CLASS` property",
            window
        );
        WmClass::get(self.aux(), window).map_or(String::from(MISSING_VALUE), |cookie| {
            cookie.reply().map_or(String::from(MISSING_VALUE), |reply| {
                str::from_utf8(reply.instance()).map_or(String::from(MISSING_VALUE), String::from)
            })
        })
    }

    /// Get a [`WindowState`] from an [`Atom`]
    pub(crate) fn get_window_state_from_atom(&self, atom: Atom) -> Option<WindowState> {
        self.win_states.get(&atom).copied()
    }

    /// Get an [`Atom`] from a [`WindowState`]
    pub(crate) const fn get_atom_from_window_state(&self, state: WindowState) -> Atom {
        match state {
            WindowState::Above => self.atoms()._NET_WM_STATE_ABOVE,
            WindowState::Below => self.atoms()._NET_WM_STATE_BELOW,
            WindowState::DemandsAttention => self.atoms()._NET_WM_STATE_DEMANDS_ATTENTION,
            WindowState::Fullscreen => self.atoms()._NET_WM_STATE_FULLSCREEN,
            WindowState::Hidden => self.atoms()._NET_WM_STATE_HIDDEN,
            WindowState::MaximizedHorz => self.atoms()._NET_WM_STATE_MAXIMIZED_HORZ,
            WindowState::MaximizedVert => self.atoms()._NET_WM_STATE_MAXIMIZED_VERT,
            WindowState::Modal => self.atoms()._NET_WM_STATE_MODAL,
            WindowState::Shaded => self.atoms()._NET_WM_STATE_SHADED,
            WindowState::SkipPager => self.atoms()._NET_WM_STATE_SKIP_PAGER,
            WindowState::SkipTaskbar => self.atoms()._NET_WM_STATE_SKIP_TASKBAR,
            WindowState::Sticky => self.atoms()._NET_WM_STATE_STICKY,
        }
    }

    // ]]] === Window State ===

    // ======================= Window Type ======================== [[[

    /// Get a [`WindowType`] from an [`Atom`]
    pub(crate) fn get_window_type_from_atom(&self, atom: Atom) -> Option<WindowType> {
        self.win_types.get(&atom).copied()
    }

    /// Get an [`Atom`] from a [`WindowType`]
    pub(crate) const fn get_atom_from_window_type(&self, r#type: WindowType) -> Atom {
        match r#type {
            WindowType::Combo => self.atoms()._NET_WM_WINDOW_TYPE_COMBO,
            WindowType::Desktop => self.atoms()._NET_WM_WINDOW_TYPE_DESKTOP,
            WindowType::Dialog => self.atoms()._NET_WM_WINDOW_TYPE_DIALOG,
            WindowType::DND => self.atoms()._NET_WM_WINDOW_TYPE_DND,
            WindowType::Dock => self.atoms()._NET_WM_WINDOW_TYPE_DOCK,
            WindowType::DropdownMenu => self.atoms()._NET_WM_WINDOW_TYPE_DROPDOWN_MENU,
            WindowType::Menu => self.atoms()._NET_WM_WINDOW_TYPE_MENU,
            WindowType::Normal => self.atoms()._NET_WM_WINDOW_TYPE_NORMAL,
            WindowType::Notification => self.atoms()._NET_WM_WINDOW_TYPE_NOTIFICATION,
            WindowType::PopupMenu => self.atoms()._NET_WM_WINDOW_TYPE_POPUP_MENU,
            WindowType::Splash => self.atoms()._NET_WM_WINDOW_TYPE_SPLASH,
            WindowType::Toolbar => self.atoms()._NET_WM_WINDOW_TYPE_TOOLBAR,
            WindowType::ToolTip => self.atoms()._NET_WM_WINDOW_TYPE_TOOLTIP,
            WindowType::Utility => self.atoms()._NET_WM_WINDOW_TYPE_UTILITY,
        }
    }

    /// Get a types of all [`Window`]s
    pub(crate) fn get_window_types(&self, window: Window) -> Vec<WindowType> {
        let mut win_types = vec![];

        if let Some(reply) = self
            .aux()
            .get_property(
                false,
                window,
                self.atoms()._NET_WM_WINDOW_TYPE,
                AtomEnum::ATOM,
                0,
                u32::MAX,
            )
            .ok()
            .and_then(|cookie| cookie.reply().ok())
        {
            if let Some(types) = reply.value32().map::<Vec<u32>, _>(Iterator::collect) {
                for type_ in types {
                    if let Some(type_) = self.get_window_type_from_atom(type_) {
                        win_types.push(type_);
                    }
                }
            }
        }

        win_types
    }

    /// Get the first type of the given [`Window`]
    pub(crate) fn get_window_preferred_type(&self, window: Window) -> WindowType {
        log::debug!("getting Window({:#0x})'s preffered type", window);
        self.get_window_types(window)
            .get(0)
            .map_or(WindowType::Normal, |&type_| type_)
    }

    // ]]] === Window Type ===

    // ========================== Pointer ========================= [[[

    /// Get the [`Point`] of the pointer
    pub(crate) fn get_pointer_position(&self) -> Point {
        log::debug!("getting pointer position");
        self.aux()
            .query_pointer(self.root())
            .map_or(Point::default(), |cookie| {
                cookie.reply().map_or(Point::default(), |reply| Point {
                    x: reply.root_x as i32,
                    y: reply.root_y as i32,
                })
            })
    }

    // /// Warp the position of the pointer
    // pub(crate) fn warp_pointer_center_of_window_or_root(&self, window:
    // Option<Window>, screen: &Monitor) {     let (pos, window) = match window
    // {         Some(window) => {
    //             let geometry = self.get_window_geometry(window);
    //
    //             if geometry.is_err() {
    //                 return;
    //             }
    //
    //             (Point::from_center_of_dim(geometry.unwrap().dim), window)
    //         },
    //         None => (
    //             Point::from_center_of_dim(screen.placeable_region().dim),
    //             self.screen.root,
    //         ),
    //     };
    //
    //     drop(
    //         self.aux()
    //             .warp_pointer(x11rb::NONE, window, 0, 0, 0, 0, pos.x as i16,
    // pos.y as i16),     );
    // }

    /// Move the position of the pointer to the given [`Point`]
    pub(crate) fn warp_pointer(&self, pnt: Point) -> Result<()> {
        log::debug!("warping pointer using Point: {:?}", pnt);
        self.aux()
            .warp_pointer(
                x11rb::NONE,
                self.root(),
                0,
                0,
                0,
                0,
                pnt.x as i16,
                pnt.y as i16,
            )
            .context("failed to `warp_pointer`")?
            .check()
            .context("failed to check warping pointer")?;

        Ok(())
    }

    /// Move position of the pointer to a [`Point`] in the given [`Window`]
    pub(crate) fn warp_pointer_win(&self, window: Window, pnt: Point) -> Result<()> {
        log::debug!("warping pointer in Window({:#0x})", window);
        self.aux()
            .warp_pointer(x11rb::NONE, window, 0, 0, 0, 0, pnt.x as i16, pnt.y as i16)
            .context("failed to `warp_pointer`")?
            .check()
            .context("failed to check warping pointer")?;

        Ok(())
    }

    /// Move the position of the pointer to the center of the screen
    pub(crate) fn center_pointer(&self, r: Rectangle) -> Result<()> {
        log::debug!("centering pointer using Rectangle: {:?}", r);
        self.aux()
            .warp_pointer(
                x11rb::NONE,
                self.root(),
                0,
                0,
                0,
                0,
                (r.point.x + r.dimension.width as i32 / 2) as i16,
                (r.point.y + r.dimension.height as i32 / 2) as i16,
            )
            .context("failed to center the pointer")?
            .check()
            .context("failed to check centering the pointer")?;

        Ok(())
    }

    /// Confine the pointer to the given [`Window`]
    // pub(crate) fn confine_pointer(&mut self, window: Window) {
    // if self.confined_to.is_none() {
    //     if let Ok(_) = self.conn.grab_pointer(
    //         false,
    //         self.screen.root,
    //         u32::from(EventMask::POINTER_MOTION |
    // EventMask::BUTTON_RELEASE) as u16,         GrabMode::
    // ASYNC,         GrabMode::ASYNC,
    //         self.screen.root,
    //         x11rb::NONE,
    //         x11rb::CURRENT_TIME,
    //     ) {
    //         drop(self.conn.grab_keyboard(
    //             false,
    //             self.screen.root,
    //             x11rb::CURRENT_TIME,
    //             GrabMode::ASYNC,
    //             GrabMode::ASYNC,
    //         ));
    //
    //         self.confined_to = Some(window);
    //     }
    // }
    // }

    // ]]] === Pointer ===

    // ]]] === Window Manager ===

    // ======================= Base Wrappers ====================== [[[

    /// Flush all pending requests to the X-Server
    pub(crate) fn flush(&self) -> bool {
        log::debug!("flushing events to the X-Server");
        if let Err(e) = self.aux().flush() {
            log::warn!("failed to flush actions to X-server: {e}");
            return false;
        }

        true
    }

    /// Synchronize events with the X-Server by flushing all pending requests to
    /// the X-Server, and then wait for the server to finish processing these
    /// requests
    pub(crate) fn sync(&self) {
        log::debug!("syncing events with the X-Server");
        if let Err(e) = self.aux().sync() {
            log::warn!("failed to sync events with X-server: {e}");
        }
    }

    /// Shorter `poll_for_event` (non-blocking)
    pub(crate) fn poll_for_event(&self) -> Option<Event> {
        log::debug!("polling for an event");
        self.aux()
            .poll_for_event()
            .context("failed to poll for next event")
            .ok()?
    }

    /// Shorter `wait_for_event` (blocking)
    pub(crate) fn wait_for_event(&self) -> Result<Event> {
        log::debug!("waiting for an event");
        self.aux()
            .wait_for_event()
            .context("failed to wait for next event")
    }

    /// Wrapper to generate an [`Xid`]
    pub(crate) fn generate_id(&self) -> Result<Xid> {
        log::debug!("generating an ID");
        self.aux().generate_id().context("failed to generate an ID")
    }

    /// Map a [`Window`], making it visible
    pub(crate) fn map_window(&self, window: Window) -> Result<()> {
        log::debug!("attempting to map Window: {}", window);
        self.aux()
            .map_window(window)
            .context(format!("failed to map window: {}", window))?
            .check()
            .context(format!("failed to check mapping window: {}", window))?;

        Ok(())
    }

    /// Unmap a [`Window`], making it visible
    pub(crate) fn unmap_window(&self, window: Window) -> Result<()> {
        log::debug!("attempting to unmap window: {}", window);
        self.aux()
            .unmap_window(window)
            .context(format!("failed to unmap window: {}", window))?
            .check()
            .context(format!("failed to check unmapping window: {}", window))?;

        Ok(())
    }

    /// Make specified window the child of the parent [`Window`]
    pub(crate) fn reparent_window(&self, window: Window, parent: Window, pnt: Point) -> Result<()> {
        log::debug!("attempting to reparent Window: {}", window);
        self.aux()
            .reparent_window(window, parent, pnt.x as i16, pnt.y as i16)
            .context(format!(
                "failed to reparent window {} to {}",
                window, parent
            ))?
            .check()
            .context(format!(
                "failed to check reparenting window {} to {}",
                window, parent
            ))?;

        Ok(())
    }

    /// Make specified [`Window`] its own parent
    pub(crate) fn unparent_window(&self, window: Window, pnt: Point) -> Result<()> {
        log::debug!("attempting to unparent Window({:#0x})", window);
        self.aux()
            .reparent_window(window, self.root(), pnt.x as i16, pnt.y as i16)
            .context(format!("failed to unparent window {}", window))?
            .check()
            .context(format!("failed to check unparenting window {}", window))?;

        Ok(())
    }

    /// Destroy the given [`Window`] and all of its sub-windows
    pub(crate) fn destroy_window(&self, window: Window) -> Result<()> {
        log::debug!("attempting to destroy Window({:#0x})", window);
        self.aux()
            .destroy_window(window)
            .context(format!("failed to destroy window {}", window))?
            .check()
            .context(format!("failed to check destroying window {}", window))?;

        Ok(())
    }

    /// Close the given [`Window`] using `WM_DELETE_WINDOW`
    pub(crate) fn close_window(&self, window: Window) -> bool {
        if self
            .send_protocol_client_message(window, self.atoms().WM_DELETE_WINDOW)
            .is_ok()
        {
            log::debug!("closed Window({:#0x})", window);
            self.flush()
        } else {
            log::debug!("failed to close Window({:#0x})", window);
            false
        }
    }

    /// Force close the given [`Window`] if `WM_DELETE_WINDOW` isn't supported
    pub(crate) fn kill_window(&self, window: Window) -> bool {
        let protocols = &[self.atoms().WM_DELETE_WINDOW];

        if self.window_supports_protocols(window, protocols) {
            self.close_window(window)
        } else if self.aux().kill_client(window).is_ok() {
            log::debug!("killed client for Window({:#0x})", window);
            self.flush()
        } else {
            log::debug!("failed to kill Window({:#0x})", window);
            false
        }
    }

    /// Cleanup atoms associated with a given [`Window`]
    pub(crate) fn cleanup_window(&self, window: Window) -> Result<()> {
        let err = |atom, window| -> String {
            format!(
                "failed to delete property `{}` for Window({:#0x})",
                atom, window
            )
        };
        let check_err = |atom, window| -> String {
            format!(
                "failed to check deleting property `{}` for Window({:#0x})",
                atom, window
            )
        };

        self.aux()
            .delete_property(window, self.atoms()._NET_WM_STATE)
            .context(err("_NET_WM_STATE", window))?
            .check()
            .context(check_err("_NET_WM_STATE", window))?;
        self.aux()
            .delete_property(window, self.atoms()._NET_WM_DESKTOP)
            .context(err("_NET_WM_DESKTOP", window))?
            .check()
            .context(check_err("_NET_WM_DESKTOP", window))?;

        Ok(())
    }

    // ]]] === Base Wrappers ===

    // ========================== Replies ========================= [[[

    /// Return information about an [`Atom`]
    pub(crate) fn intern_atom<S>(&self, only_if_exists: bool, name: S) -> Result<InternAtomReply>
    where
        S: AsRef<str>,
    {
        log::debug!("interning an atom: {}", name.as_ref());
        self.aux()
            .intern_atom(only_if_exists, name.as_ref().as_bytes())
            .context("failed to get `InternAtomReply`")?
            .reply()
            .context("failed to get `InternAtomReply` reply")
    }

    /// Wrapper to change window attributes
    pub(crate) fn change_window_attributes(
        &self,
        value_list: &ChangeWindowAttributesAux,
    ) -> Result<()> {
        log::debug!("changing window attribute");
        self.aux()
            .change_window_attributes(self.root(), value_list)
            .context("failed to change window attributes")?
            .check()
            .context("failed to check after changing window attributes")?;

        Ok(())
    }

    /// Wrapper for getting a [`Window`]'s attributes
    pub(crate) fn get_window_attributes(&self, window: Window) -> Result<GetWindowAttributesReply> {
        log::debug!("requesting a `GetWindowAttributesReply` reply");
        self.aux()
            .get_window_attributes(window)
            .context("failed to get `GetWindowAttributesReply`")?
            .reply()
            .context("failed to get `GetWindowAttributesReply` reply")
    }

    /// Wrapper for getting a [`Window`]'s geometry
    pub(crate) fn get_geometry(&self, window: Window) -> Result<GetGeometryReply> {
        log::debug!("requesting a `GetGeometryReply` reply");
        self.aux()
            .get_geometry(window)
            .context("failed to get `GetGeometryReply`")?
            .reply()
            .context("failed to get `GetGeometryReply` reply")
    }

    /// Return the information about the focused [`Window`](xproto::Window)
    ///
    /// # Errors
    /// Will cause an error if requesting `GetInputFocusReply` fails
    pub(crate) fn get_input_focus(&self) -> Result<GetInputFocusReply> {
        log::debug!("requesting a `GetInputFocusReply` reply");
        self.aux()
            .get_input_focus()
            .context("failed to get `GetInputFocusReply`")?
            .reply()
            .context("failed to get `GetInputFocusReply` reply")
    }

    /// Return the owner of the given [`Atom`]
    pub(crate) fn get_selection_owner(&self, atom: Atom) -> Result<GetSelectionOwnerReply> {
        log::debug!("requesting a `GetSelectionOwnerReply` reply");
        self.aux()
            .get_selection_owner(atom)
            .context("failed to get `GetSelectionOwnerReply`")?
            .reply()
            .context("failed to get `GetSelectionOwnerReply` reply")
    }

    /// Get a [`WindowState`] from an [`Atom`]
    pub(crate) fn get_atom_name(&self, atom: Atom) -> Result<GetAtomNameReply> {
        log::debug!("requesting a `GetAtomNameReply` reply");
        self.aux()
            .get_atom_name(atom)
            .context("failed to get `GetAtomNameReply`")?
            .reply()
            .context("failed to get `GetAtomNameReply` reply")
    }

    /// Return result of querying the [`Window`] tree
    pub(crate) fn query_tree(&self, window: Window) -> Result<QueryTreeReply> {
        log::debug!("requesting a `QueryTreeReply` reply");
        self.aux()
            .query_tree(window)
            .context("failed to get `QueryTreeReply`")?
            .reply()
            .context("failed to get `QueryTreeReply` reply")
    }

    /// Return pointer's window and its coordinates
    pub(crate) fn query_pointer(&self, window: Window) -> Result<QueryPointerReply> {
        log::debug!("requesting a `QueryTreeReply` reply");
        self.aux()
            .query_pointer(window)
            .context("failed to get `QueryPointerReply`")?
            .reply()
            .context("failed to get `QueryPointerReply` reply")
    }

    /// Delete the given property from the `root`
    pub(crate) fn delete_property(&self, property: Atom) -> Result<()> {
        log::debug!("deleting property: `{}`", property);
        self.aux()
            .delete_property(self.root(), property)
            .context(format!("failed to `delete_property`: `{}`", property))?
            .check()
            .context(format!("failed to check `delete_property`: `{}`", property))?;

        Ok(())
    }

    // ]]] === Replies ===

    // ======================== Grab / Ungrab ===================== [[[

    /// Grab control of all keyboard input
    pub(crate) fn grab_keyboard(&self) -> Result<()> {
        log::debug!("attempting to grab control of the entire keyboard");
        let reply = self
            .aux()
            .grab_keyboard(
                false,       // owner events
                self.root(), // window
                x11rb::CURRENT_TIME,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )
            .context("failed to grab keyboard")?
            .reply()
            .context("failed to get reply after grabbing keyboard")?;

        if reply.status == xproto::GrabStatus::ALREADY_GRABBED {
            return Err(anyhow!("the keyboard is already grabbed"));
        } else if reply.status != xproto::GrabStatus::SUCCESS {
            return Err(anyhow!(
                "failed to grab keyboard. Replied with unsuccessful status"
            ));
        }

        Ok(())
    }

    /// Ungrab/release the keyboard
    pub(crate) fn ungrab_keyboard(&self) -> Result<()> {
        log::debug!("attempting to ungrab control of the entire keyboard");
        self.aux()
            .ungrab_keyboard(x11rb::CURRENT_TIME)
            .context("failed to ungrab keyboard")?
            .check()
            .context("failed to check ungrabbing keyboard")?;

        Ok(())
    }

    /// Regrab all buttons in the given [`Window`]
    pub(crate) fn regrab_buttons(&self, window: Window) -> Result<()> {
        self.aux()
            .grab_button(
                true,
                window,
                u32::from(self.masks.regrab_event_mask) as u16,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
                x11rb::NONE,
                x11rb::NONE,
                ButtonIndex::ANY,
                ModMask::ANY,
            )
            .context("failed to regrab buttons")?
            .check()
            .context("failed to check regrabbing button")?;

        Ok(())
    }

    /// Ungrab all currently grabbed buttons
    pub(crate) fn ungrab_buttons(&self, window: Window) -> Result<()> {
        self.aux()
            .ungrab_button(ButtonIndex::ANY, window, ModMask::ANY)
            .context("failed to ungrab all buttons")?
            .check()
            .context("failed to check ungrabbing all buttons")?;

        Ok(())
    }

    /// Ungrab the pointer
    #[allow(clippy::unwrap_used)]
    pub(crate) fn grab_pointer(&self, window: Window, confine: Option<Window>) -> Result<()> {
        log::debug!("attempting to grab control of the pointer");
        self.aux().grab_pointer(
            false,
            window,
            u32::from(EventMask::POINTER_MOTION | EventMask::BUTTON_RELEASE) as u16,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
            t!(confine.is_some() ? confine.unwrap() : x11rb::NONE),
            x11rb::NONE,
            x11rb::CURRENT_TIME,
        )?;
        Ok(())
    }

    /// Grab the server (wrapper for errors)
    pub(crate) fn grab_server(&self) -> Result<()> {
        log::debug!("grabbing the server");
        self.aux()
            .grab_server()
            .context("failed to grab server")?
            .check()
            .context("failed to check grabbing server")?;
        Ok(())
    }

    /// Ungrab the server (wrapper for errors)
    pub(crate) fn ungrab_server(&self) -> Result<()> {
        self.aux()
            .ungrab_server()
            .context("failed to ungrab server")?;
        Ok(())
    }

    // ]]] === Grab/Ungrab ===

    // ========================== Stack =========================== [[[

    /// Stack given [`Window`] above or below other windows, with a sibling
    pub(crate) fn stack_window(
        &self,
        mode: xproto::StackMode,
        window: Window,
        sibling: Option<Window>,
    ) -> Result<()> {
        let mut aux = ConfigureWindowAux::default().stack_mode(mode);

        if let Some(sibling) = sibling {
            aux = aux.sibling(sibling);
        }

        self.aux()
            .configure_window(window, &aux)
            .context(format!("failed to stack Window({:#0x}) {:?}", window, mode))?
            .check()
            .context(format!(
                "failed to check stacking Window({:#0x}) {:?}",
                window, mode
            ))?;

        Ok(())
    }

    /// Stack given [`Window`] above other windows. Bring a sibling along
    #[allow(clippy::unwrap_used)]
    pub(crate) fn stack_window_above(&self, window: Window, sibling: Option<Window>) -> Result<()> {
        log::debug!(
            "stacking Window({:#0x}){} above",
            window,
            t!(sibling.is_some()
                ? format!(" with Sibling({:#0x})", sibling.unwrap())
                : String::from(""))
        );
        let mut aux = ConfigureWindowAux::default().stack_mode(xproto::StackMode::ABOVE);

        if let Some(sibling) = sibling {
            aux = aux.sibling(sibling);
        }

        self.aux()
            .configure_window(window, &aux)
            .context(format!("failed to stack Window({:#0x}) below", window))?
            .check()
            .context(format!(
                "failed to check stacking Window({:#0x}) below",
                window
            ))?;

        Ok(())
    }

    /// Stack given [`Window`] below other windows. Bring a sibling along
    #[allow(clippy::unwrap_used)]
    pub(crate) fn stack_window_below(&self, window: Window, sibling: Option<Window>) -> Result<()> {
        log::debug!(
            "stacking Window({:#0x}){} below",
            window,
            t!(sibling.is_some()
                ? format!(" with Sibling({:#0x})", sibling.unwrap())
                : String::from(""))
        );
        let mut aux = ConfigureWindowAux::default().stack_mode(xproto::StackMode::BELOW);

        if let Some(sibling) = sibling {
            aux = aux.sibling(sibling);
        }

        self.aux()
            .configure_window(window, &aux)
            .context(format!("failed to stack Window({:#0x}) below", window))?
            .check()
            .context(format!(
                "failed to check stacking Window({:#0x}) below",
                window
            ))?;

        Ok(())
    }

    /// Put [`Window`] into the client's `save_set`
    pub(crate) fn insert_window_in_save_set(&self, window: Window) -> Result<()> {
        self.aux()
            .change_save_set(xproto::SetMode::INSERT, window)
            .context(format!(
                "failed to `change_save_set` for Window({:#0x})",
                window
            ))?
            .check()
            .context(format!(
                "failed to check `change_save_set` for Window({:#0x})",
                window
            ))?;

        Ok(())
    }

    // ]]] === Stack ===

    // =========================== Helper ========================= [[[

    /// Get the supported [`Atoms`]
    pub(crate) fn get_supported(&self) -> Result<HashMap<Atom, bool>> {
        log::debug!("getting supported Atoms");
        // TODO: Does this need to be a hash?
        let mut supported = HashMap::new();
        let reply = self
            .aux()
            .get_property(
                false,
                self.root(),
                self.atoms()._NET_SUPPORTED,
                self.atoms().ATOM, // AtomEnum::ATOM,
                0,
                u32::MAX,
            )
            .context("failed to get property: '_NET_SUPPORTED'")?
            .reply()
            .context("failed to get reply from: '_NET_SUPPORTED'")?;

        for atom in reply
            .value32()
            .ok_or_else(|| Error::InvalidProperty(String::from("_NET_SUPPORTED")))?
        {
            supported.insert(atom, true);
        }

        Ok(supported)
    }

    /// Check that the used extensions are installed and that the versions are
    /// up to date
    fn check_extensions(conn: &RustConnection) -> Result<()> {
        log::debug!("checking that extensions are installed");
        let use_extension = |conn: &RustConnection, extension_name: &'static str| -> Result<()> {
            if conn.extension_information(extension_name)?.is_none() {
                lwm_fatal!(
                    "{} X11 extension is unsupported",
                    extension_name.green().bold()
                );
            }
            Ok(())
        };

        // Check `xkb` extension
        use_extension(conn, xkb::X11_EXTENSION_NAME)?;
        let (min, max) = xkb::X11_XML_VERSION;
        if let Err(e) = conn.xkb_use_extension(min as u16, max as u16) {
            lwm_fatal!(
                "`xkb` version is unsupported. Supported versions: {}-{}: {}",
                min,
                max,
                e
            );
        };
        log::debug!("`xkb` extension is up to date: {}-{}", min, max);

        // Check `randr` extension
        use_extension(conn, randr::X11_EXTENSION_NAME)?;
        let (min, max) = randr::X11_XML_VERSION;
        if let Err(e) = conn.randr_query_version(min, max) {
            lwm_fatal!(
                "`randr` version is unsupported. Supported versions: {}-{}: {}",
                min,
                max,
                e
            );
        };
        log::debug!("`randr` extension is up to date: {}-{}", min, max);

        Ok(())
    }

    // ]]] === Helper ===

    // ========================== Running ========================= [[[

    /// Check if another composite manager is running
    pub(crate) fn composite_manager_running(&self) -> Result<bool> {
        log::debug!("checking if another composite manager is running");
        let atom = format!("_NET_WM_CM_S{}", self.screen());
        let atom = self.intern_atom(false, atom)?.atom;
        let owner = self.get_selection_owner(atom)?;
        Ok(owner.owner != x11rb::NONE)
    }

    // TODO: Use or delete
    /// Scan for already existing windows and manage them
    pub(crate) fn scan_windows(&mut self) -> Result<()> {
        let tree = self.query_tree(self.root())?;

        for win in tree.children {
            let attr = self.get_window_attributes(win);
            let geom = self.get_geometry(win);

            if let (Ok(attr), Ok(geom)) = (attr, geom) {
                if !attr.override_redirect && attr.map_state != MapState::UNMAPPED {
                    self.manage_window(win, &geom)?;
                }
            }
        }

        Ok(())
    }

    // ]]] === Running ===

    // =========================== Other ========================== [[[

    // ================= Initialization Expanded ================== [[[

    /// Initialize supported [`Atom`]s
    fn init_supported(&self) -> Result<()> {
        self.aux()
            .change_property32(
                PropMode::REPLACE,
                self.root(),
                self.atoms()._NET_SUPPORTED,
                self.atoms().ATOM,
                &[
                    self.atoms()._NET_ACTIVE_WINDOW,
                    self.atoms()._NET_CLIENT_LIST,
                    self.atoms()._NET_CLIENT_LIST_STACKING,
                    self.atoms()._NET_CLOSE_WINDOW,
                    self.atoms()._NET_CURRENT_DESKTOP,
                    self.atoms()._NET_DESKTOP_NAMES,
                    self.atoms()._NET_DESKTOP_VIEWPORT,
                    self.atoms()._NET_MOVERESIZE_WINDOW,
                    self.atoms()._NET_NUMBER_OF_DESKTOPS,
                    self.atoms()._NET_SUPPORTED,
                    self.atoms()._NET_SUPPORTING_WM_CHECK,
                    self.atoms()._NET_WM_DESKTOP,
                    self.atoms()._NET_MOVERESIZE_WINDOW,
                    self.atoms()._NET_WM_MOVERESIZE,
                    self.atoms()._NET_WM_NAME,
                    self.atoms()._NET_WM_STATE,
                    self.atoms()._NET_WM_STATE_DEMANDS_ATTENTION,
                    self.atoms()._NET_WM_STATE_FOCUSED,
                    self.atoms()._NET_WM_STATE_FULLSCREEN,
                    self.atoms()._NET_WM_STATE_HIDDEN,
                    self.atoms()._NET_WM_STATE_MODAL,
                    self.atoms()._NET_WM_STATE_STICKY,
                    self.atoms()._NET_WM_STRUT_PARTIAL,
                    self.atoms()._NET_WM_VISIBLE_NAME,
                    self.atoms()._NET_WM_WINDOW_TYPE,
                    self.atoms()._NET_WM_WINDOW_TYPE_DIALOG,
                    self.atoms()._NET_WM_WINDOW_TYPE_DOCK,
                    self.atoms()._NET_WM_WINDOW_TYPE_DROPDOWN_MENU,
                    self.atoms()._NET_WM_WINDOW_TYPE_MENU,
                    self.atoms()._NET_WM_WINDOW_TYPE_NORMAL,
                    self.atoms()._NET_WM_WINDOW_TYPE_NOTIFICATION,
                    self.atoms()._NET_WM_WINDOW_TYPE_POPUP_MENU,
                    self.atoms()._NET_WM_WINDOW_TYPE_SPLASH,
                    self.atoms()._NET_WM_WINDOW_TYPE_TOOLBAR,
                    self.atoms()._NET_WM_WINDOW_TYPE_TOOLTIP,
                    self.atoms()._NET_WM_WINDOW_TYPE_UTILITY,
                ],
            )
            .context("failed to initialize supported `_NET_SUPPORTED`")?
            .check()
            .context("failed to check `_NET_SUPPORTED`")?;

        Ok(())
    }

    // ]]] === Initialization Expanded ===

    // ========================= Cleanup ========================== [[[
    /// Cleanup everything having to do with the window manager
    ///
    /// Errors do not need to be checked here
    pub(crate) fn cleanup(self) {
        log::debug!("ungrabbing all keys");
        drop(
            self.aux()
                .ungrab_key(xproto::Grab::ANY, self.root(), xproto::ModMask::ANY),
        );

        drop(self.destroy_window(self.meta_window()));
        drop(self.clear_input_focus());

        for atom in [
            self.atoms()._NET_ACTIVE_WINDOW,
            self.atoms()._NET_SUPPORTING_WM_CHECK,
            self.atoms()._NET_WM_NAME,
            self.atoms().WM_CLASS,
            self.atoms()._NET_SUPPORTED,
            self.atoms()._NET_WM_PID,
            self.atoms()._NET_CLIENT_LIST,
        ] {
            log::debug!("deleting property: {}", atom);
            drop(self.delete_property(atom));
        }

        // NOTE: Is stdout still connected here?
        log::debug!("dropping the connection. Goodbye");
        drop(self.conn);
    }

    /// Release the pointer from being confined to a [`Window`]
    #[allow(clippy::unused_self)]
    pub(crate) fn release_pointer(&mut self) {
        // if self.confined_to.is_some() {
        //     drop(self.conn.ungrab_pointer(x11rb::CURRENT_TIME));
        //     drop(self.conn.ungrab_keyboard(x11rb::CURRENT_TIME));
        //
        //     self.confined_to = None;
        // }
    }

    /// ]]] === Cleanup ===

    // ========================= Retrieve ========================= [[[

    /// Get the number of desktops using `_NET_NUMBER_OF_DESKTOPS`
    pub(crate) fn get_num_desktops(&self) -> Result<u32> {
        log::debug!("requesting property `_NET_NUMBER_OF_DESKTOPS`");
        let num = self
            .aux()
            .get_property(
                false,
                self.root(),
                self.atoms()._NET_NUMBER_OF_DESKTOPS,
                AtomEnum::CARDINAL,
                0,
                u32::MAX,
            )
            .context("failed to get property: `_NET_NUMBER_OF_DESKTOPS`")?
            .reply()
            .context("failed to get property reply: `_NET_NUMBER_OF_DESKTOPS`")?
            .value32()
            .and_then(|mut i| i.next())
            .ok_or_else(|| Error::InvalidProperty(String::from("_NET_NUMBER_OF_DESKTOPS")))?;

        Ok(num)
    }

    /// Get the currently active window's ID using `_NET_ACTIVE_WINDOW`
    pub(crate) fn get_active_window_id(&self) -> Result<Xid> {
        log::debug!("requesting property `_NET_ACTIVE_WINDOW`");
        Ok(self
            .aux()
            .get_property(
                false,
                self.root(),
                self.atoms()._NET_ACTIVE_WINDOW,
                AtomEnum::WINDOW,
                0,
                u32::MAX,
            )
            .context("failed to get property: `_NET_ACTIVE_WINDOW`")?
            .reply()
            .context("failed to get property reply: `_NET_ACTIVE_WINDOW`")?
            .value32()
            .and_then(|mut i| i.next())
            .ok_or_else(|| Error::InvalidProperty(String::from("_NET_ACTIVE_WINDOW")))?)
    }

    /// Get the parent of a given [`Window`]
    pub(crate) fn get_window_parent(&self, window: Window) -> Result<u32> {
        let tree = self.query_tree(window)?;
        let id = tree.parent;
        log::debug!("getting Window({:#0x})'s parent: {}", window, id);
        Ok(id)
    }

    /// Get a vector of the [`Window`]'s [`Strut`]s from `_NET_WM_STRUT`
    pub(crate) fn get_window_strut(&self, window: Window) -> Option<Vec<Option<Strut>>> {
        if let Some(strut) = self.get_window_strut_partial(window) {
            return Some(strut);
        }

        self.aux()
            .get_property(
                false,
                window,
                self.atoms()._NET_WM_STRUT,
                self.atoms().CARDINAL,
                0,
                u32::MAX,
            )
            .ok()?
            .reply()
            .ok()
            .and_then(|strut| {
                let widths: Vec<u32> = strut.value32()?.collect();
                if widths.is_empty() {
                    return None;
                }

                let mut struts = Vec::with_capacity(4);
                for (i, &width) in widths.iter().enumerate() {
                    if i == 4 {
                        break;
                    }

                    struts.push((width != 0).then(|| Strut { window, width }));
                }

                Some(struts)
            })
    }

    /// Get the `_NET_WM_STRUT_PARTIAL` atom
    pub(crate) fn get_window_strut_partial(&self, window: Window) -> Option<Vec<Option<Strut>>> {
        log::debug!("getting `_NET_WM_STRUT_PARTIAL` for Window({:#0x})", window);
        self.aux()
            .get_property(
                false,
                window,
                self.atoms()._NET_WM_STRUT_PARTIAL,
                self.atoms().CARDINAL,
                0,
                u32::MAX,
            )
            .ok()?
            .reply()
            .ok()
            .and_then(|strut_partial| {
                let widths: Vec<u32> = strut_partial.value32()?.collect();
                if widths.is_empty() {
                    return None;
                }

                let mut struts = Vec::with_capacity(1);
                for (i, &width) in widths.iter().enumerate() {
                    if i == 4 {
                        break;
                    }

                    struts.push((width != 0).then(|| Strut { window, width }));
                }

                Some(struts)
            })
    }

    // TODO: Use or delete
    /// Get the window manager's process ID use `_NET_WM_PID`
    pub(crate) fn get_window_pid1(&self, window: Window) -> Result<u32> {
        log::debug!("requesting property `_NET_WM_PID`");
        Ok(self
            .aux()
            .get_property(
                false,
                window,
                self.atoms()._NET_WM_PID,
                AtomEnum::CARDINAL,
                0,
                u32::MAX,
            )
            .context("failed to get property: `_NET_WM_PID`")?
            .reply()
            .context("failed to get property reply: `_NET_WM_PID`")?
            .value32()
            .and_then(|mut i| i.next())
            .ok_or_else(|| Error::InvalidProperty(String::from("_NET_WM_PID")))?)
    }

    /// Get the window manager's process ID use `_NET_WM_PID`
    pub(crate) fn get_window_pid(&self, window: Window) -> Option<Pid> {
        use protocol::res;

        let id_spec = res::ClientIdSpec {
            client: window,
            mask:   u8::from(res::ClientIdMask::LOCAL_CLIENT_PID) as u32,
        };

        res::query_client_ids(self.aux(), &[id_spec])
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|reply| {
                for i in reply.ids {
                    if (i.spec.mask & (u8::from(res::ClientIdMask::LOCAL_CLIENT_PID)) as u32) != 0
                        && !i.value.is_empty()
                        && i.value[0] != 0
                    {
                        return Some(i.value[0]);
                    }
                }

                None
            })
    }

    // /// Get the list of connected [`Monitor`]s
    // pub(crate) fn connected_outputs(&self) -> Vec<Monitor> {
    //     let resources = self.aux().randr_get_screen_info(self.meta_window());
    //
    //     if let Ok(resources) = resources {
    //         if let Ok(reply) = resources.reply() {
    //             return reply
    //                 .crtcs
    //                 .iter()
    //                 .flat_map(|crtc| {
    //                     randr::get_crtc_info(self.conn, *crtc, 0)
    //                         .map(|cookie| cookie.reply().map(|reply| reply))
    //                 })
    //                 .enumerate()
    //                 .map(|(i, r)| {
    //                     let r = r.unwrap();
    //                     let region = Region {
    //                         pos: Pos { x: r.x as i32, y: r.y as i32 },
    //                         dim: Dim {
    //                             w: r.width as u32,
    //                             h: r.height as u32,
    //                         },
    //                     };
    //
    //                     Screen::new(region, i)
    //                 })
    //                 .filter(|screen| screen.full_region().dim.w > 0)
    //                 .collect();
    //         }
    //     }
    //
    //     panic!("could not obtain screen resources")
    // }

    /// Get the top-level [`Window`]s
    pub(crate) fn get_top_level_windows(&self) -> Vec<Window> {
        log::debug!("getting top-level windows");
        self.query_tree(self.root()).map_or(vec![], |reply| {
            reply
                .children
                .iter()
                .filter(|&w| self.must_manage_window(*w))
                .copied()
                .collect()
        })
    }

    /// Get the [`Window`]'s geometry, returning a [`Rectangle`]
    pub(crate) fn get_window_geometry(&self, window: Window) -> Result<Rectangle> {
        log::debug!("getting geometry for Window({:#0x})", window);
        // translate_coordinates must be used to get the actual values
        let geom = self.get_geometry(window)?;

        let trans = self
            .aux()
            .translate_coordinates(window, self.root(), geom.x, geom.y)
            .context(format!(
                "failed to get `TranslateCoordinatesReply` of window: {}",
                window
            ))?
            .reply()
            .context("failed to get `TranslateCoordinatesReply` reply")?;

        let (x, y, w, h) = (trans.dst_x, trans.dst_y, geom.width, geom.height);
        log::debug!(
            "Window({:#0x}): Geomtry: x: {}, y: {}, w: {}, h: {}",
            window,
            x,
            y,
            w,
            h
        );

        Ok(Rectangle::new(x as i32, y as i32, w as u32, h as u32))
    }

    /// Get the window's [`Hints`]
    pub(crate) fn get_icccm_window_hints(&self, window: Window) -> Option<Hints> {
        log::debug!("getting `Hints` for Window({:#0x})", window);
        let hints = properties::WmHints::get(self.aux(), window)
            .ok()?
            .reply()
            .ok()?;

        Some(Hints {
            urgent:        hints.urgent,
            input:         hints.input,
            group:         hints.window_group,
            initial_state: hints.initial_state.map(IcccmWindowState::from),
        })
    }

    /// Get the [`Window`]s [`SizeHints`]
    pub(crate) fn get_icccm_window_size_hints(
        &self,
        window: Window,
        min_dim: Option<Dimension>,
        current_size_hints: &Option<SizeHints>,
    ) -> (bool, Option<SizeHints>) {
        log::debug!("setting `SizeHints` for window {}", window);
        let size_hints = properties::WmSizeHints::get_normal_hints(self.aux(), window)
            .ok()
            .and_then(|cookie| cookie.reply().ok());

        if size_hints.is_none() {
            return (current_size_hints.is_none(), None);
        }

        #[allow(clippy::unwrap_used)]
        let size_hints = size_hints.unwrap();

        let (by_user, position) = size_hints.position.map_or((false, None), |(spec, x, y)| {
            (
                matches!(spec, properties::WmSizeHintsSpecification::UserSpecified),
                (x > 0_i32 || y > 0_i32).then(|| Point { x, y }),
            )
        });

        let (sh_min_width, sh_min_height) =
            size_hints.min_size.map_or((None, None), |(width, height)| {
                (
                    (width > 0_i32).then(|| width as u32),
                    (height > 0_i32).then(|| height as u32),
                )
            });

        let (sh_base_width, sh_base_height) =
            size_hints
                .base_size
                .map_or((None, None), |(width, height)| {
                    (
                        (width > 0_i32).then(|| width as u32),
                        (height > 0_i32).then(|| height as u32),
                    )
                });

        let (max_width, max_height) = size_hints.max_size.map_or((None, None), |(width, height)| {
            (
                (width > 0_i32).then(|| width as u32),
                (height > 0_i32).then(|| height as u32),
            )
        });

        let min_width = t!(sh_min_width.is_some() ? sh_min_width : sh_base_width);
        let min_height = t!(sh_min_height.is_some() ? sh_min_height : sh_base_height);
        let base_width = t!(sh_base_width.is_some() ? sh_base_width : sh_min_width);
        let base_height = t!(sh_base_height.is_some() ? sh_base_height : sh_min_height);

        #[rustfmt::skip]
        let min_width = min_width.and_then(|min_width| {
            min_dim.map_or(
                (min_width > 0).then(|| min_width),
                |min_dim| Some(t!(min_width >= min_dim.width ? min_width : min_dim.width))
            )
        });

        #[rustfmt::skip]
        let min_height = min_height.and_then(|min_height| {
            min_dim.map_or(
                (min_height > 0).then(|| min_height),
                |min_dim| Some(t!(min_height >= min_dim.height ? min_height : min_dim.height))
            )
        });

        let (inc_width, inc_height) =
            size_hints
                .size_increment
                .map_or((None, None), |(inc_width, inc_height)| {
                    (
                        (inc_width > 0_i32 && inc_width < 0xFFFF_i32).then(|| inc_width as u32),
                        (inc_width > 0_i32 && inc_width < 0xFFFF_i32).then(|| inc_width as u32),
                    )
                });

        let ((min_ratio, max_ratio), (min_ratio_vulgar, max_ratio_vulgar)) = size_hints
            .aspect
            .map_or(((None, None), (None, None)), |(min_ratio, max_ratio)| {
                (
                    (
                        (min_ratio.numerator > 0_i32 && min_ratio.denominator > 0_i32)
                            .then(|| (min_ratio.numerator / min_ratio.denominator) as f64),
                        (min_ratio.numerator > 0_i32 && min_ratio.denominator > 0_i32)
                            .then(|| (min_ratio.numerator / min_ratio.denominator) as f64),
                    ),
                    (
                        Some(Ratio {
                            numerator:   min_ratio.numerator,
                            denominator: min_ratio.denominator,
                        }),
                        Some(Ratio {
                            numerator:   max_ratio.numerator,
                            denominator: max_ratio.denominator,
                        }),
                    ),
                )
            });

        let size_hints = Some(SizeHints {
            by_user,
            position,
            base_width,
            base_height,
            min_width,
            min_height,
            max_width,
            max_height,
            inc_width,
            inc_height,
            min_ratio,
            max_ratio,
            min_ratio_vulgar,
            max_ratio_vulgar,
        });

        (*current_size_hints == size_hints, size_hints)
    }

    /// Get the focused window
    ///
    /// Same as [`get_input_focus`](#method.get_input_focus), minus errors
    pub(crate) fn get_focused_window(&self) -> Window {
        self.aux().get_input_focus().map_or(self.root(), |cookie| {
            cookie.reply().map_or(self.root(), |reply| reply.focus)
        })
    }

    /// Get the `WM_TRANSIENT_FOR` atom
    pub(crate) fn get_icccm_window_transient_for(&self, window: Window) -> Option<Window> {
        self.aux()
            .get_property(
                false,
                window,
                self.atoms().WM_TRANSIENT_FOR,
                self.atoms().WINDOW,
                0,
                u32::MAX,
            )
            .ok()?
            .reply()
            .ok()
            .and_then(|transient_for| {
                let transient_for: Vec<u32> = transient_for.value32()?.collect();

                if transient_for.is_empty() {
                    log::debug!("`WM_TRANSIENT_FOR` is empty");
                    None
                } else {
                    // TODO: Possibly just return transient_for[0]
                    if transient_for[0] == 0 {
                        log::debug!("`WM_TRANSIENT_FOR` is 0");
                        None
                    } else {
                        log::debug!("got `WM_TRANSIENT_FOR` for Window({:#0x})", window);
                        Some(transient_for[0])
                    }
                }
            })
    }

    /// Get the [`Window`]'s `WM_CLIENT_LEADER` atom
    /// TODO: Use or delete
    pub(crate) fn get_icccm_window_client_leader(&self, window: Window) -> Option<Window> {
        self.aux()
            .get_property(
                false,
                window,
                self.atoms().WM_CLIENT_LEADER,
                self.atoms().WINDOW,
                0,
                u32::MAX,
            )
            .ok()?
            .reply()
            .ok()
            .and_then(|client_leader| {
                let client_leader: Vec<u32> = client_leader.value32()?.collect();

                if client_leader.is_empty() {
                    None
                } else {
                    Some(client_leader[0])
                }
            })
    }

    // ]]] === Retrieve ===

    // =========================== Set ============================ [[[

    /// Set the root [`Window`]'s name
    pub(crate) fn set_root_window_name(&self, name: &str) -> Result<()> {
        log::debug!("setting `WM_NAME`: {}", name);
        self.aux()
            .change_property8(
                PropMode::REPLACE,
                self.root(),
                self.atoms().WM_NAME,
                self.atoms().UTF8_STRING,
                name.as_bytes(),
            )
            .context("failed to change `WM_NAME`")?
            .check()
            .context("failed to check changing `WM_NAME`")?;

        Ok(())
    }

    /// Set the current desktop using and index
    pub(crate) fn set_current_desktop(&self, idx: usize) -> Result<()> {
        log::debug!("setting `_NET_CURRENT_DESKTOP`: {}", idx);
        self.set_atom(
            self.root(),
            self.atoms()._NET_CURRENT_DESKTOP,
            &[idx as u32],
        )?;

        Ok(())
    }

    /// Set the desktop of the given [`Window`]
    pub(crate) fn set_window_desktop(&self, window: Window, idx: usize) -> Result<()> {
        log::debug!(
            "setting `_NET_WM_DESKTOP` for Window({:#0x}) to desktop {}",
            window,
            idx
        );
        self.set_atom(window, self.atoms()._NET_WM_DESKTOP, &[idx as u32])?;

        Ok(())
    }

    /// Set the [`Window`]s [`Hints`]
    pub(crate) fn set_icccm_window_hints(&self, window: Window, hints: Hints) -> Result<()> {
        log::debug!("setting `Hints` for Window({:#0x})", window);
        let wm_hints = properties::WmHints {
            input:         hints.input,
            initial_state: hints
                .initial_state
                .and_then(IcccmWindowState::to_wmhintsstate),
            icon_pixmap:   None,
            icon_window:   None,
            icon_position: None,
            icon_mask:     None,
            window_group:  hints.group,
            urgent:        hints.urgent,
        };

        wm_hints
            .set(self.aux(), window)
            .context(format!(
                "failed to set Window({:#0x}) for `WmHints`",
                window
            ))?
            .check()
            .context(format!(
                "failed to check setting window {} for `WmHints`",
                window
            ))?;

        Ok(())
    }

    /// Set the [`Window`]'s [`Extents`] (padding)
    pub(crate) fn set_window_frame_extents(&self, window: Window, extents: Extents) -> Result<()> {
        log::debug!("setting `Extents` for Window({:#0x})", window);
        let frame_extents = vec![extents.left, extents.right, extents.top, extents.bottom];

        self.set_atom(window, self.atoms()._NET_FRAME_EXTENTS, &frame_extents[..])?;

        Ok(())
    }

    /// Set `_NET_DESKTOP_GEOMETRY`
    pub(crate) fn set_desktop_geometry(&self, geometries: &[&Rectangle]) -> Result<()> {
        log::debug!("setting `_NET_DESKTOP_GEOMETRY`");
        self.set_atom(
            self.root(),
            self.atoms()._NET_DESKTOP_GEOMETRY,
            &geometries.iter().fold(Vec::new(), |mut acc, geometry| {
                acc.push(geometry.point.x as u32);
                acc.push(geometry.point.y as u32);
                acc.push(geometry.dimension.width);
                acc.push(geometry.dimension.height);
                acc
            }),
        )
    }

    /// Set `_NET_DESKTOP_VIEWPORT`
    pub(crate) fn set_desktop_viewport(&self, viewports: &[&Rectangle]) -> Result<()> {
        log::debug!("setting `_NET_DESKTOP_VIEWPORT`");
        self.set_atom(
            self.root(),
            self.atoms()._NET_DESKTOP_VIEWPORT,
            &viewports.iter().fold(Vec::new(), |mut acc, viewport| {
                acc.push(viewport.point.x as u32);
                acc.push(viewport.point.y as u32);
                acc.push(viewport.dimension.width);
                acc.push(viewport.dimension.height);
                acc
            }),
        )
    }

    /// Set `_NET_WORKAREA`
    pub(crate) fn set_workarea(&self, workareas: &[&Rectangle]) -> Result<()> {
        log::debug!("setting `_NET_WORKAREA`");
        self.set_atom(
            self.root(),
            self.atoms()._NET_WORKAREA,
            &workareas.iter().fold(Vec::new(), |mut acc, workarea| {
                acc.push(workarea.point.x as u32);
                acc.push(workarea.point.y as u32);
                acc.push(workarea.dimension.width);
                acc.push(workarea.dimension.height);
                acc
            }),
        )
    }

    // ]]] === Set ===

    // ]]] === Other ===

    /// Debugging method
    fn print_data_type(reply: &GetPropertyReply) {
        println!("Reply: {:#?}", reply);
        println!("DataType: {:#?}", AtomEnum::from(reply.type_ as u8));
    }
} // ]]] === XConnection ===
