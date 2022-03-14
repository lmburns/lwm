//! Interacting directly with the X11 server

use crate::{
    config::Config,
    error::Error,
    lwm_fatal,
    types::{
        Atom,
        Button,
        IcccmProps,
        IcccmWindowState,
        Window,
        WindowClass,
        WindowMap,
        WindowState,
        WindowType,
        Xid,
        MISSING_VALUE,
        TITLEBAR_HEIGHT,
    },
    WM_NAME,
};
use anyhow::{Context, Result};
use itertools::Itertools;
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    convert::TryFrom,
    process,
    str::{self, FromStr},
    sync::Arc,
};
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
            ChangeWindowAttributesAux,
            ClientMessageEvent,
            ConnectionExt,
            CreateGCAux,
            CreateWindowAux,
            EventMask,
            GetGeometryReply,
            GetInputFocusReply,
            GetPropertyReply,
            GetSelectionOwnerReply,
            GetWindowAttributesReply,
            InputFocus,
            InternAtomReply,
            MapState,
            ModMask,
            PropMode,
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

use nix::unistd;

// === Atoms === [[[

/// An [`Atom`] is a unique ID corresponding to a string name that is used to
/// identify properties, types, and selections. See the [Client Properties][1]
/// and [Extended Properties][2] for more information.
///
/// [1]: https://specifications.freedesktop.org/wm-spec/wm-spec-latest.html#idm45381393900464
/// [2]: https://tronche.com/gui/x/icccm/sec-4.html#s-4.1.2
atom_manager! {
    pub Atoms: AtomsCookie {
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

        // ==== ICCCM client properties ====
        WM_NAME,
        // Consecutive null-term strings; Instance and class
        WM_CLASS,
        // ID of another top-level window. Pop-up on behalf of window
        WM_TRANSIENT_FOR,
        // Forms name of machine running the client
        WM_CLIENT_MACHINE,
        // List of atoms identifying protocol between client and window
        WM_PROTOCOLS,
        // Type is WM_SIZE_HINTS
        WM_NORMAL_HINTS,
        // Has atom if prompt of deletion or deletion is about to happen
        WM_DELETE_WINDOW,
        WM_WINDOW_ROLE,
        WM_CLIENT_LEADER,
        // Window may receieve a `ClientMessage` event
        WM_TAKE_FOCUS,

        // ==== ICCCM window manager properties ====
        // Top-level windows not in withdrawn have this tag
        WM_STATE,
        // If wishes to place constrains on sizes of icon pixmaps
        WM_ICON_SIZE,

        // === EWMH root properties ===
        // Indicates which hints are supported
        _NET_SUPPORTED,
        // Set on root window to be the ID of a child window to indicate WM is active
        _NET_SUPPORTING_WM_CHECK,
        // All windows managed by the window manager
        _NET_CLIENT_LIST,
        // Array of null-terminated strings for all virtual desktops
        _NET_DESKTOP_NAMES,
        // Array of pairs of cardinals define top-left corner of each desktop viewport
        _NET_DESKTOP_VIEWPORT,
        // Indicate number of virtual desktops
        _NET_NUMBER_OF_DESKTOPS,
        // Window ID of active window or none if no window is focused
        _NET_ACTIVE_WINDOW,

        // no
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
        _NET_SHOWING_DESKTOP,

        // === EWMH root messages ===
        // Wanting to close a window muse send this request
        _NET_CLOSE_WINDOW,

        // no
        _NET_MOVERESIZE_WINDOW,
        _NET_WM_MOVERESIZE,
        _NET_REQUEST_FRAME_EXTENTS,

        // === EWMH application properties ===
        _NET_WM_STRUT_PARTIAL,
        _NET_WM_DESKTOP,
        _NET_WM_STATE,
        _NET_WM_WINDOW_TYPE,

        // no
        _NET_WM_NAME,
        _NET_WM_VISIBLE_NAME,
        _NET_WM_ICON_NAME,
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
        _NET_WM_STATE_FOCUSED,

        // === EWMH window types ===
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
        _NET_WM_WINDOW_TYPE_NORMAL,

        // EWMH protocols
        _NET_WM_PING,
        _NET_WM_SYNC_REQUEST,
        _NET_WM_FULLSCREEN_MONITORS,

        // System tray protocols
        _NET_SYSTEM_TRAY_ORIENTATION,
        _NET_SYSTEM_TRAY_OPCODE,
        _NET_SYSTEM_TRAY_ORIENTATION_HORZ,
        _NET_SYSTEM_TRAY_S0,
        _XEMBED,
        _XEMBED_INFO,
    }
}
// ]]] === Atoms ===

// black_gc: Gcontext,
// windows: Vec<WindowState>,
// pending_expose: HashSet<Window>,
// wm_protocols: Atom,
// wm_delete_window: Atom,
// sequences_to_ignore: BinaryHeap<Reverse<u16>>,
// drag_window: Option<(Window, (i16, i16))>,

/// The main connection to the X-Server
#[derive(Clone)]
pub(crate) struct LWM {
    /// The actual [`Connection`](RustConnection)
    conn:        Arc<RustConnection>,
    /// The [`Atoms`] of the connection
    atoms:       Atoms,
    /// Generated ID
    meta_window: Window,
    /// A hash mapping an [`Atom`] to a [`WindowType`]
    win_types:   HashMap<Atom, WindowType>,
    /// A hash mapping an [`Atom`] to a [`WindowState`]
    win_states:  HashMap<Atom, WindowState>,
    /// Screen number the connection is attached to
    screen:      usize,
    /// State of the window manager
    restart:     bool,
    /// Background graphics context
    gctx:        xproto::Gcontext,
    //-
    // // Information about the current [`Screen`](xproto::Screen)
    // screen:       xproto::Screen,
    // // The X11 resource database (`xrdb`)
    // database:      Option<Database>,
    // // A confined X11 [`Connection`](xproto::Connection) ??
    // confined_to:   Option<Window>
}

impl LWM {
    /// Create a new [`LWM`]
    pub(crate) fn new(conn: RustConnection, screen_num: usize, config: &Config) -> Result<Self> {
        Self::check_extensions(&conn).context("failed to query extensions")?;
        let setup = conn.setup();
        let screen = setup.roots[screen_num].clone();
        let root = screen.root;

        let screen_width = screen.width_in_pixels;
        let screen_height = screen.height_in_pixels;

        let meta_window = conn.generate_id().context("failed to generate an `ID`")?;
        let gctx = conn.generate_id().context("failed to generate an `ID`")?;

        // Allocate a graphics context
        conn.create_gc(gctx, root, &CreateGCAux::new())?
            .check()
            .context("create graphics context")?;

        // conn.grab_server()
        //     .context("failed to grab server")?
        //     .check()
        //     .context("failed to check after grabbing server")?;

        log::debug!("interning Atoms");
        let atoms = Atoms::new(&conn)
            .context("failed to get `Atoms`")?
            .reply()
            .context("failed to get `Atoms` reply")?;

        let mut xconn = Self {
            conn: Arc::new(conn),
            atoms,
            screen: screen_num,
            meta_window,
            restart: false,
            win_types: WindowType::to_hashmap(&atoms),
            win_states: WindowState::to_hashmap(&atoms),
            gctx,
        };

        // xconn.init(config)?;

        // xconn.become_wm()?;
        // xconn
        //     .conn
        //     .ungrab_server()
        //     .context("failed to ungrab server")?
        //     .check()
        //     .context("failed to check after ungrabbing server")?;

        Ok(xconn)
    }

    // /// Initialize the window manager
    // fn init(&self, config: &Config) -> Result<()> {
    //     self.init_window()?;
    //
    //     let desktop_names = config.global.desktops.clone().unwrap_or_else(|| {
    //         (1_i32..=5_i32)
    //             .into_iter()
    //             .map(|d| format!("{d}"))
    //             .collect_vec()
    //     });
    //
    //     self.init_properties(WM_NAME!(), &desktop_names[..]);
    //
    //     Ok(())
    // }

    // ========================== TESTING ==========================
    // ========================== TESTING ==========================
    /// testing func
    #[allow(clippy::unnecessary_wraps)]
    pub(crate) fn get_test(&self) -> Result<()> {
        log::debug!("requesting a `GetScreenSizeRangeReply` reply");

        let reply = self
            .conn
            .get_property(
                false,
                self.root(),
                self.atoms._NET_NUMBER_OF_DESKTOPS,
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
    pub(crate) fn connection(&self) -> &RustConnection {
        &self.conn
    }

    /// Return the `root` window
    pub(crate) fn root(&self) -> xproto::Window {
        self.conn.setup().roots[self.screen].root
    }

    /// Return the focused screen number
    pub(crate) const fn screen(&self) -> usize {
        self.screen
    }

    // ]]] === Accessor ===

    // ======================== Initialize ======================== [[[

    /// Initialize the meta window
    fn init_window(&self) -> Result<()> {
        self.conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            self.meta_window,
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

        self.grab_server()?;
        self.conn.map_window(self.meta_window);
        self.ungrab_server()?;

        Ok(())
    }

    /// Initializes the database and set the cursor
    fn init_cursor(&self) {
        if let Ok(ref db) =
            Database::new_from_default(self.connection()).context("failed to get database")
        {
            CursorHandle::new(self.connection(), self.screen, db).map(|cookie| {
                cookie.reply().map(|inner| {
                    let aux = ChangeWindowAttributesAux::default()
                        .cursor(inner.load_cursor(self.connection(), "left_ptr").ok());

                    self.change_window_attributes(&aux);
                })
            });
        }
    }

    /// Initializes the wanted window manager properties
    fn init_properties<S: AsRef<str>>(&self, wm_name: &str, desktop_names: &[S]) -> Result<()> {
        // Specifies instance and class names, separated by null
        // TODO: Possible each null terminated?
        let instance_class_names = &[wm_name, wm_name];
        let wm_class = instance_class_names.join("\0");

        self.conn
            .change_property8(
                PropMode::REPLACE,
                self.meta_window,
                self.atoms._NET_WM_NAME,
                self.atoms.UTF8_STRING,
                wm_name.as_bytes(),
            )
            .context("failed to replace `_NET_WM_NAME`")?
            .check()
            .context("failed to check replacing `_NET_WM_NAME`")?;

        // set_icccm_window_class
        self.conn
            .change_property8(
                PropMode::REPLACE,
                self.meta_window,
                self.atoms.WM_CLASS,
                self.atoms.UTF8_STRING,
                wm_class.as_bytes(),
            )
            .context("failed to replace `WM_CLASS`")?
            .check()
            .context("failed to check replacing `WM_CLASS`")?;

        self.conn
            .change_property32(
                PropMode::REPLACE,
                self.meta_window,
                self.atoms._NET_WM_PID,
                self.atoms.CARDINAL,
                &[process::id()],
            )
            .context("failed to replace `_NET_WM_PID`")?
            .check()
            .context("failed to check replacing `_NET_WM_PID`")?;

        self.conn
            .change_property32(
                PropMode::REPLACE,
                self.root(),
                self.atoms._NET_SUPPORTING_WM_CHECK,
                self.atoms.WINDOW,
                &[self.meta_window],
            )
            .context("failed to replace `_NET_SUPPORTING_WM_CHECK`")?
            .check()
            .context("failed to check replacing `_NET_SUPPORTING_WM_CHECK`")?;

        self.conn
            .change_property8(
                PropMode::REPLACE,
                self.root(),
                self.atoms._NET_WM_NAME,
                self.atoms.UTF8_STRING,
                wm_name.as_bytes(),
            )
            .context("failed to replace `_NET_WM_NAME`")?
            .check()
            .context("failed to check replacing `_NET_WM_NAME`")?;

        self.conn
            .change_property8(
                PropMode::REPLACE,
                self.root(),
                self.atoms.WM_CLASS,
                self.atoms.UTF8_STRING,
                wm_class.as_bytes(),
            )
            .context("failed to replace `WM_CLASS`")?
            .check()
            .context("failed to check replacing `WM_CLASS`")?;

        self.conn
            .change_property32(
                PropMode::REPLACE,
                self.meta_window,
                self.atoms._NET_SUPPORTING_WM_CHECK,
                self.atoms.WINDOW,
                &[self.meta_window],
            )
            .context("failed to replace `_NET_SUPPORTING_WM_CHECK`")?
            .check()
            .context("failed to check replacing `_NET_SUPPORTING_WM_CHECK`")?;

        self.init_supported()?;

        self.conn
            .change_property32(
                PropMode::REPLACE,
                self.root(),
                self.atoms._NET_WM_PID,
                self.atoms.CARDINAL,
                &[process::id()],
            )
            .context("failed to replace `_NET_WM_PID`")?
            .check()
            .context("failed to check replacing `_NET_WM_PID`")?;

        self.conn
            .delete_property(self.root(), self.atoms._NET_CLIENT_LIST)
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
        self.change_window_attributes(
            &ChangeWindowAttributesAux::new().event_mask(EventMask::SUBSTRUCTURE_REDIRECT),
        )
        .context("another window manager is currently running")?;
        Ok(())
    }

    /// Send a [`ClientMessageEvent`]
    pub(crate) fn send_client_message(&self, window: Window, atom: Atom, type_: Atom) -> Result<()> {
        let data = [atom, x11rb::CURRENT_TIME, 0, 0, 0];
        let event = ClientMessageEvent::new(32, window, type_, data);

        self.conn
            .send_event(false, window, EventMask::NO_EVENT, &event)
            .context(format!(
                "failed to send event. Window: {}, Type: {}",
                event.window, event.type_
            ))?
            .check()
            .context(format!(
                "failed to check sending event. Window: {}, Type: {}",
                event.window, event.type_
            ))?;

        // Is this needed?
        self.flush();

        Ok(())
    }

    /// Send a [`ClientMessageEvent`] using `WM_PROTOCOLS`
    pub(crate) fn send_protocol_client_message(&self, window: Window, atom: Atom) -> Result<()> {
        self.send_client_message(window, atom, self.atoms.WM_PROTOCOLS)
    }

    // ====================== Testing Values ====================== [[[

    /// Check whether the window supports any `WM_PROTOCOLS`
    pub(crate) fn window_supports_protocols(&self, window: Window, protocols: &[Atom]) -> bool {
        self.conn
            .get_property(
                false,
                window,
                self.atoms.WM_PROTOCOLS,
                self.atoms.ATOM, // AtomEnum::ATOM,
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
    pub(crate) fn window_is_any_of_state(&self, window: Window, states: &[Atom]) -> bool {
        self.conn
            .get_property(
                false,
                window,
                self.atoms._NET_WM_STATE,
                self.atoms.ATOM, // AtomEnum::ATOM,
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
        self.conn
            .get_property(
                false,
                window,
                self.atoms._NET_WM_WINDOW_TYPE,
                self.atoms.ATOM, // AtomEnum::ATOM,
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
        let do_not_manage = self
            .conn
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
            self.atoms._NET_WM_WINDOW_TYPE_DOCK,
            self.atoms._NET_WM_WINDOW_TYPE_TOOLBAR,
        ];

        !self.window_is_any_of_types(window, to_exclude)
    }

    /// Check if the given [`Window`] is mappable
    pub(crate) fn window_is_mappable(&self, window: Window) -> bool {
        self.conn
            .get_window_attributes(window)
            .map_or(false, |cookie| {
                cookie.reply().map_or(false, |reply| {
                    let default_state = properties::WmHintsState::Normal;
                    let initial_state = properties::WmHints::get(self.connection(), window)
                        .ok()
                        .map_or(default_state, |cookie| {
                            cookie.reply().map_or(default_state, |reply| {
                                reply.initial_state.map_or(default_state, |i| i)
                            })
                        });

                    reply.class != xproto::WindowClass::INPUT_ONLY
                        && !self.window_is_any_of_state(window, &[self.atoms._NET_WM_STATE_HIDDEN])
                        && matches!(initial_state, properties::WmHintsState::Normal)
                })
            })
    }

    // ]]] ===== Testing Values =====

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
                .conn
                .get_property(
                    false,
                    self.root(),
                    self.atoms._NET_CLIENT_LIST,
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
        let reply = self
            .conn
            .get_property(
                false,
                window,
                self.atoms.WM_CLASS,
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
        let class = std::str::from_utf8(&iter.collect::<Vec<_>>())?.to_owned();
        log::debug!("WindowClass: id: {}, class: {}", window, class);
        Ok(class)
    }

    // ========================= Actions === ====================== [[[

    /// Create a new handle for a [`Window`]
    pub(crate) fn create_handle(&self) -> Result<Window> {
        let wid = self
            .conn
            .generate_id()
            .context("failed to generate an ID")?;
        let aux = xproto::CreateWindowAux::new().override_redirect(1);

        self.conn
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
            .context(format!("failed to create window: {}", wid))?
            .check()
            .context(format!("failed check creating window: {}", wid))?;

        self.flush();

        Ok(wid)
    }

    /// Focus the given [`Window`]
    pub(crate) fn focus_window(&self, window: Window) -> Result<()> {
        self.conn
            .set_input_focus(InputFocus::PARENT, window, x11rb::CURRENT_TIME)
            .context(format!("failed to `set_input_focus`: {}", window))?
            .check()
            .context(format!("failed to check `set_input_focus`: {}", window))?;

        self.conn
            .change_property32(
                PropMode::REPLACE,
                self.root(),
                self.atoms._NET_ACTIVE_WINDOW,
                AtomEnum::WINDOW,
                &[window],
            )
            .context("failed to replace property `_NET_ACTIVE_WINDOW`")?
            .check()
            .context("failed to replace property `_NET_ACTIVE_WINDOW`")?;

        Ok(())
    }

    /// TODO DOCUMENT
    #[inline]
    pub(crate) fn unfocus(&self) -> Result<()> {
        log::debug!("unfocusing `meta_window`");
        self.conn
            .set_input_focus(InputFocus::PARENT, self.meta_window, x11rb::CURRENT_TIME)
            .context("failed to unfocus `meta_window`");

        self.delete_property(self.atoms._NET_ACTIVE_WINDOW)?;

        Ok(())
    }

    /// Add a new window that should be managed by the WM
    pub(crate) fn manage_window(&mut self, win: Window, geom: &GetGeometryReply) -> Result<()> {
        log::debug!("Managing window {:?}", win);
        let screen = &self.conn.setup().roots[self.screen];
        // assert!(self.find_window_by_id(win).is_none());

        let frame_win = self.conn.generate_id()?;
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

        self.conn.create_window(
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
        self.conn.change_save_set(SetMode::INSERT, win)?;
        let cookie = self
            .conn
            .reparent_window(win, frame_win, 0, TITLEBAR_HEIGHT as _)?;
        self.conn.map_window(win)?;
        self.conn.map_window(frame_win)?;
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

    // ========================== Update ========================== [[[

    /// Change the name of the desktops
    fn update_desktops<S: AsRef<str>>(&self, desktop_names: &[S]) -> Result<()> {
        log::debug!("updating `_NET_NUMBER_OF_DESKTOPS`");
        self.conn
            .change_property32(
                PropMode::REPLACE,
                self.root(),
                self.atoms._NET_NUMBER_OF_DESKTOPS,
                self.atoms.CARDINAL,
                &[desktop_names.len() as u32],
            )
            .context("failed to replace `_NET_NUMBER_OF_DESKTOPS`")?
            .check()
            .context("failed to check replacing `_NET_NUMBER_OF_DESKTOPS`")?;

        log::debug!("updating `_NET_DESKTOP_NAMES`");
        self.conn
            .change_property8(
                PropMode::REPLACE,
                self.root(),
                self.atoms._NET_DESKTOP_NAMES,
                self.atoms.UTF8_STRING,
                desktop_names
                    .iter()
                    .map(AsRef::as_ref)
                    .join("\0")
                    .as_bytes(),
            )
            .context("failed to replace `_NET_DESKTOP_NAMES`")?
            .check()
            .context("failed to check replacing `_NET_DESKTOP_NAMES`")?;

        Ok(())
    }

    // ]]] === Update ===

    // ]]] === Actions ===

    // ======================= Window State ======================= [[[

    /// Get a [`Window`]s state
    pub(crate) fn get_window_state(&self, win: Window) -> Result<Vec<WindowState>> {
        log::debug!("querying for window {}'s state", win);
        let mut states = vec![];
        let reply = self
            .conn
            .get_property(
                false,
                win,
                self.atoms._NET_WM_STATE,
                AtomEnum::ATOM,
                0,
                u32::MAX,
            )
            .context("failed to get property: `_NET_WM_STATE`")?
            .reply()
            .context("failed to get property: `_NET_WM_STATE` reply")?;
        for state in reply
            .value32()
            .ok_or_else(|| Error::InvalidProperty(String::from("_NET_WM_STATE")))?
        {
            let state = WindowState::from_atoms(&self.atoms, state)?;
            log::debug!("WindowState: id: {}, state: {}", win, state);
            states.push(state);
        }
        Ok(states)
    }

    /// Set the `_NET_WM_STATE` property
    pub(crate) fn set_window_state_atom(
        &self,
        window: Window,
        state_atom: Atom,
        on: bool,
    ) -> Result<()> {
        if on {
            log::debug!("setting `on` window state atom");
            if self.window_is_any_of_state(window, &[state_atom]) {
                return Ok(());
            }

            self.conn
                .change_property32(
                    PropMode::APPEND,
                    window,
                    self.atoms._NET_WM_STATE,
                    AtomEnum::ATOM,
                    &[state_atom],
                )
                .context("failed to append property `_NET_WM_STATE`")?
                .check()
                .context("failed to append property `_NET_WM_STATE`")?;

            Ok(())
        } else {
            log::debug!("setting window state atom");
            let mut states = self
                .conn
                .get_property(
                    false,
                    window,
                    self.atoms._NET_WM_STATE,
                    self.atoms.ATOM,
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

            self.conn
                .change_property32(
                    PropMode::REPLACE,
                    window,
                    self.atoms._NET_WM_STATE,
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
        self.set_window_state_atom(
            window,
            match state {
                WindowState::Modal => self.atoms._NET_WM_STATE_MODAL,
                WindowState::Sticky => self.atoms._NET_WM_STATE_STICKY,
                WindowState::MaximizedVert => self.atoms._NET_WM_STATE_MAXIMIZED_VERT,
                WindowState::MaximizedHorz => self.atoms._NET_WM_STATE_MAXIMIZED_HORZ,
                WindowState::Shaded => self.atoms._NET_WM_STATE_SHADED,
                WindowState::SkipTaskbar => self.atoms._NET_WM_STATE_SKIP_TASKBAR,
                WindowState::SkipPager => self.atoms._NET_WM_STATE_SKIP_PAGER,
                WindowState::Hidden => self.atoms._NET_WM_STATE_HIDDEN,
                WindowState::Fullscreen => self.atoms._NET_WM_STATE_FULLSCREEN,
                WindowState::Above => self.atoms._NET_WM_STATE_ABOVE,
                WindowState::Below => self.atoms._NET_WM_STATE_BELOW,
                WindowState::DemandsAttention => self.atoms._NET_WM_STATE_DEMANDS_ATTENTION,
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
        log::debug!("setting icccm window {}'s state to {:?}", window, state);
        self.conn
            .change_property32(
                xproto::PropMode::REPLACE,
                window,
                self.atoms.WM_STATE,
                self.atoms.CARDINAL,
                &[state.into(), 0],
            )
            .context("failed to set `IcccmWindowState`")?
            .check()
            .context("failed to check setting `IcccmWindowState`")?;

        Ok(())
    }

    /// Get an `icccm` window's `WM_STATE` property
    pub(crate) fn get_icccm_window_class(&self, window: Window) -> String {
        WmClass::get(self.connection(), window).map_or(String::from(MISSING_VALUE), |cookie| {
            cookie.reply().map_or(String::from(MISSING_VALUE), |reply| {
                str::from_utf8(reply.class()).map_or(String::from(MISSING_VALUE), String::from)
            })
        })
    }

    /// Get an `icccm` window's `WM_NAME` property
    pub(crate) fn get_icccm_window_name(&self, window: Window) -> String {
        self.conn
            .get_property(
                false,
                window,
                self.atoms.WM_NAME,
                self.atoms.UTF8_STRING,
                0,
                u32::MAX,
            )
            .map_or(String::from(MISSING_VALUE), |cookie| {
                cookie.reply().map_or(String::from(MISSING_VALUE), |reply| {
                    str::from_utf8(&reply.value8().map_or(Vec::new(), Iterator::collect))
                        .map_or(String::from(MISSING_VALUE), ToOwned::to_owned)
                })
            })
    }

    /// Get an `icccm` window's name contained in the `WM_CLASS` property
    pub(crate) fn get_icccm_window_instance(&self, window: Window) -> String {
        WmClass::get(self.connection(), window).map_or(String::from(MISSING_VALUE), |cookie| {
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
            WindowState::Above => self.atoms._NET_WM_STATE_ABOVE,
            WindowState::Below => self.atoms._NET_WM_STATE_BELOW,
            WindowState::DemandsAttention => self.atoms._NET_WM_STATE_DEMANDS_ATTENTION,
            WindowState::Fullscreen => self.atoms._NET_WM_STATE_FULLSCREEN,
            WindowState::Hidden => self.atoms._NET_WM_STATE_HIDDEN,
            WindowState::MaximizedHorz => self.atoms._NET_WM_STATE_MAXIMIZED_HORZ,
            WindowState::MaximizedVert => self.atoms._NET_WM_STATE_MAXIMIZED_VERT,
            WindowState::Modal => self.atoms._NET_WM_STATE_MODAL,
            WindowState::Shaded => self.atoms._NET_WM_STATE_SHADED,
            WindowState::SkipPager => self.atoms._NET_WM_STATE_SKIP_PAGER,
            WindowState::SkipTaskbar => self.atoms._NET_WM_STATE_SKIP_TASKBAR,
            WindowState::Sticky => self.atoms._NET_WM_STATE_STICKY,
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
            WindowType::Combo => self.atoms._NET_WM_WINDOW_TYPE_COMBO,
            WindowType::Desktop => self.atoms._NET_WM_WINDOW_TYPE_DESKTOP,
            WindowType::Dialog => self.atoms._NET_WM_WINDOW_TYPE_DIALOG,
            WindowType::DND => self.atoms._NET_WM_WINDOW_TYPE_DND,
            WindowType::Dock => self.atoms._NET_WM_WINDOW_TYPE_DOCK,
            WindowType::DropdownMenu => self.atoms._NET_WM_WINDOW_TYPE_DROPDOWN_MENU,
            WindowType::Menu => self.atoms._NET_WM_WINDOW_TYPE_MENU,
            WindowType::Normal => self.atoms._NET_WM_WINDOW_TYPE_NORMAL,
            WindowType::Notification => self.atoms._NET_WM_WINDOW_TYPE_NOTIFICATION,
            WindowType::PopupMenu => self.atoms._NET_WM_WINDOW_TYPE_POPUP_MENU,
            WindowType::Splash => self.atoms._NET_WM_WINDOW_TYPE_SPLASH,
            WindowType::Toolbar => self.atoms._NET_WM_WINDOW_TYPE_TOOLBAR,
            WindowType::ToolTip => self.atoms._NET_WM_WINDOW_TYPE_TOOLTIP,
            WindowType::Utility => self.atoms._NET_WM_WINDOW_TYPE_UTILITY,
        }
    }

    // ]]] === Window Type ===

    // ]]] === Window Manager ===

    // ======================= Base Wrappers ====================== [[[

    /// Flush all pending requests to the X-Server
    pub(crate) fn flush(&self) {
        log::debug!("flushing events to the X-Server");
        if let Err(e) = self.conn.flush() {
            log::warn!("failed to flush actions to X-server: {e}");
        }
    }

    /// Synchronize events with the X-Server by flushing all pending requests to
    /// the X-Server, and then wait for the server to finish processing these
    /// requests
    pub(crate) fn sync(&self) {
        log::debug!("syncing events with the X-Server");
        if let Err(e) = self.conn.sync() {
            log::warn!("failed to sync events with X-server: {e}");
        }
    }

    /// Shorter `poll_for_event` (non-blocking)
    pub(crate) fn poll_for_event(&self) -> Option<Event> {
        log::debug!("polling for an event");
        self.conn
            .poll_for_event()
            .context("failed to poll for next event")
            .ok()?
    }

    /// Shorter `wait_for_event` (blocking)
    pub(crate) fn wait_for_event(&self) -> Result<Event> {
        log::debug!("waiting for an event");
        self.conn
            .wait_for_event()
            .context("failed to wait for next event")
    }

    /// Wrapper to generate an [`Xid`]
    pub(crate) fn generate_id(&self) -> Result<Xid> {
        self.conn.generate_id().context("failed to generate an ID")
    }

    /// Map a [`Window`], making it visible
    pub(crate) fn map_window(&self, window: Window) -> Result<()> {
        self.conn
            .map_window(window)
            .context(format!("failed to map window: {}", window))?
            .check()
            .context(format!("failed to check mapping window: {}", window))?;

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
        self.conn
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
        self.conn
            .change_window_attributes(self.root(), value_list)
            .context("failed to change window attributes")?
            .check()
            .context("failed to check after changing window attributes")?;

        Ok(())
    }

    /// Wrapper for getting a [`Window`]'s attributes
    pub(crate) fn get_window_attributes(&self, window: Window) -> Result<GetWindowAttributesReply> {
        self.conn
            .get_window_attributes(window)
            .context("failed to get `GetWindowAttributesReply`")?
            .reply()
            .context("failed to get `GetWindowAttributesReply` reply")
    }

    /// Wrapper for getting a [`Window`]'s geometry
    pub(crate) fn get_geometry(&self, window: Window) -> Result<GetGeometryReply> {
        self.conn
            .get_geometry(window)
            .context("failed to get `GetGeometryReply`")?
            .reply()
            .context("failed to get `GetGeometryReply` reply")
    }

    /// Return the information about the focused [`Window`](xproto::Window)
    pub(crate) fn get_input_focus(&self) -> Result<GetInputFocusReply> {
        log::debug!("requesting a `GetInputFocusReply` reply");
        self.conn
            .get_input_focus()
            .context("failed to get `GetInputFocusReply`")?
            .reply()
            .context("failed to get `GetInputFocusReply` reply")
    }

    /// Return the owner of the given [`Atom`]
    pub(crate) fn get_selection_owner(&self, atom: Atom) -> Result<GetSelectionOwnerReply> {
        log::debug!("requesting a `GetSelectionOwnerReply` reply");
        self.conn
            .get_selection_owner(atom)
            .context("failed to get `GetSelectionOwnerReply`")?
            .reply()
            .context("failed to get `GetSelectionOwnerReply` reply")
    }

    /// Return result of querying the [`Window`] tree
    pub(crate) fn query_tree(&self, window: Window) -> Result<QueryTreeReply> {
        log::debug!("requesting a `QueryTreeReply` reply");
        self.conn
            .query_tree(window)
            .context("failed to get `QueryTreeReply`")?
            .reply()
            .context("failed to get `QueryTreeReply` reply")
    }

    /// Delete the given property from the `root`
    pub(crate) fn delete_property(&self, property: Atom) -> Result<()> {
        self.conn
            .delete_property(self.root(), property)
            .context(format!("failed to `delete_property`: {}", property))?
            .check()
            .context(format!("failed to check `delete_property`: {}", property))?;

        Ok(())
    }

    // ]]] === Replies ===

    // ======================== Grab / Ungrab ===================== [[[

    /// Grab control of all keyboard input
    pub(crate) fn grab_keyboard(&self) -> Result<()> {
        log::debug!("attempting to grab control of the entire keyboard");
        let reply = self
            .conn
            .grab_keyboard(
                false,       // owner events
                self.root(), // window
                x11rb::CURRENT_TIME,
                xproto::GrabMode::ASYNC,
                xproto::GrabMode::ASYNC,
            )
            .context("failed to grab keyboard")?
            .reply()
            .context("failed to get reply after grabbing keyboard")?;

        if reply.status == xproto::GrabStatus::ALREADY_GRABBED {
            log::info!("the keyboard is already grabbed");
        } else if reply.status != xproto::GrabStatus::SUCCESS {
            lwm_fatal!("failed to grab keyboard. Replied with unsuccessful status");
        }

        Ok(())
    }

    /// Ungrab/release the keyboard
    pub(crate) fn ungrab_keyboard(&self) {
        log::debug!("attempting to ungrab control of the entire keyboard");
        if let Err(e) = self.conn.ungrab_keyboard(x11rb::CURRENT_TIME) {
            lwm_fatal!("failed to ungrab keyboard: {}", e);
        }
    }

    /// Grab the server (wrapper for errors)
    pub(crate) fn grab_server(&self) -> Result<()> {
        self.conn.grab_server().context("failed to grab server")?;
        Ok(())
    }

    /// Ungrab the server (wrapper for errors)
    pub(crate) fn ungrab_server(&self) -> Result<()> {
        self.conn
            .ungrab_server()
            .context("failed to ungrab server")?;
        Ok(())
    }

    // ]]] === Grab/Ungrab ===

    // =========================== Helper ========================= [[[

    /// Get the supported [`Atoms`]
    pub(crate) fn get_supported(&self) -> Result<HashMap<Atom, bool>> {
        log::debug!("getting supported Atoms");
        // TODO: Does this need to be a hash?
        let mut supported = HashMap::new();
        let reply = self
            .conn
            .get_property(
                false,
                self.root(),
                self.atoms._NET_SUPPORTED,
                self.atoms.ATOM, // AtomEnum::ATOM,
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
        let atom = format!("_NET_WM_CM_S{}", self.screen);
        let atom = self.intern_atom(false, atom)?.atom;
        let owner = self.get_selection_owner(atom)?;
        Ok(owner.owner != x11rb::NONE)
    }

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
        self.conn
            .change_property32(
                PropMode::REPLACE,
                self.root(),
                self.atoms._NET_SUPPORTED,
                self.atoms.ATOM,
                &[
                    self.atoms._NET_ACTIVE_WINDOW,
                    self.atoms._NET_CLIENT_LIST,
                    self.atoms._NET_CLIENT_LIST_STACKING,
                    self.atoms._NET_CLOSE_WINDOW,
                    self.atoms._NET_CURRENT_DESKTOP,
                    self.atoms._NET_DESKTOP_NAMES,
                    self.atoms._NET_DESKTOP_VIEWPORT,
                    self.atoms._NET_MOVERESIZE_WINDOW,
                    self.atoms._NET_NUMBER_OF_DESKTOPS,
                    self.atoms._NET_SUPPORTED,
                    self.atoms._NET_SUPPORTING_WM_CHECK,
                    self.atoms._NET_WM_DESKTOP,
                    self.atoms._NET_MOVERESIZE_WINDOW,
                    self.atoms._NET_WM_MOVERESIZE,
                    self.atoms._NET_WM_NAME,
                    self.atoms._NET_WM_STATE,
                    self.atoms._NET_WM_STATE_DEMANDS_ATTENTION,
                    self.atoms._NET_WM_STATE_FOCUSED,
                    self.atoms._NET_WM_STATE_FULLSCREEN,
                    self.atoms._NET_WM_STATE_HIDDEN,
                    self.atoms._NET_WM_STATE_MODAL,
                    self.atoms._NET_WM_STATE_STICKY,
                    self.atoms._NET_WM_STRUT_PARTIAL,
                    self.atoms._NET_WM_VISIBLE_NAME,
                    self.atoms._NET_WM_WINDOW_TYPE,
                    self.atoms._NET_WM_WINDOW_TYPE_DIALOG,
                    self.atoms._NET_WM_WINDOW_TYPE_DOCK,
                    self.atoms._NET_WM_WINDOW_TYPE_DROPDOWN_MENU,
                    self.atoms._NET_WM_WINDOW_TYPE_MENU,
                    self.atoms._NET_WM_WINDOW_TYPE_NORMAL,
                    self.atoms._NET_WM_WINDOW_TYPE_NOTIFICATION,
                    self.atoms._NET_WM_WINDOW_TYPE_POPUP_MENU,
                    self.atoms._NET_WM_WINDOW_TYPE_SPLASH,
                    self.atoms._NET_WM_WINDOW_TYPE_TOOLBAR,
                    self.atoms._NET_WM_WINDOW_TYPE_TOOLTIP,
                    self.atoms._NET_WM_WINDOW_TYPE_UTILITY,
                ],
            )
            .context("failed to initialize supported `_NET_SUPPORTED`")?
            .check()
            .context("failed to check `_NET_SUPPORTED`")?;

        Ok(())
    }

    // ]]] === Initialization Expanded ===

    // ========================= Retrieve ========================= [[[

    /// Get the number of desktops using `_NET_NUMBER_OF_DESKTOPS`
    pub(crate) fn get_num_desktops(&self) -> Result<u32> {
        log::debug!("requesting property `_NET_NUMBER_OF_DESKTOPS`");
        let num = self
            .conn
            .get_property(
                false,
                self.root(),
                self.atoms._NET_NUMBER_OF_DESKTOPS,
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
            .conn
            .get_property(
                false,
                self.root(),
                self.atoms._NET_ACTIVE_WINDOW,
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
        log::debug!("WindowParent: id: {}, parent: {}", window, id);
        Ok(id)
    }

    /// Get the window manager's process ID use `_NET_WM_PID`
    pub(crate) fn get_window_pid(&self, window: Window) -> Result<u32> {
        log::debug!("requesting property `_NET_WM_PID`");
        Ok(self
            .conn
            .get_property(
                false,
                window,
                self.atoms._NET_WM_PID,
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

    // ]]] === Retrieve ===

    // =========================== Set ============================ [[[

    /// Set the root [`Window`]'s name
    pub(crate) fn set_root_window_name(&self, name: &str) -> Result<()> {
        log::debug!("setting root window name: {}", name);
        self.conn
            .change_property8(
                PropMode::REPLACE,
                self.root(),
                self.atoms.WM_NAME,
                self.atoms.UTF8_STRING,
                name.as_bytes(),
            )
            .context("failed to change `WM_NAME`")?
            .check()
            .context("failed to check changing `WM_NAME`")?;

        Ok(())
    }

    /// Set the current desktop using and index
    pub(crate) fn set_current_desktop(&self, idx: usize) -> Result<()> {
        log::debug!("setting current desktop: {}", idx);
        self.conn
            .change_property32(
                PropMode::REPLACE,
                self.root(),
                self.atoms._NET_CURRENT_DESKTOP,
                self.atoms.CARDINAL,
                &[idx as u32],
            )
            .context("failed to change `_NET_CURRENT_DESKTOP`")?
            .check()
            .context("failed to check changing `_NET_CURRENT_DESKTOP`")?;

        Ok(())
    }

    /// Set the desktop of the given [`Window`]
    pub(crate) fn set_window_desktop(&self, window: Window, idx: usize) -> Result<()> {
        log::debug!("setting window {} to desktop {}", window, idx);
        self.conn
            .change_property32(
                PropMode::REPLACE,
                window,
                self.atoms._NET_WM_DESKTOP,
                self.atoms.CARDINAL,
                &[idx as u32],
            )
            .context("failed to change `_NET_WM_DESKTOP`")?
            .check()
            .context("failed to check changing `_NET_WM_DESKTOP`")?;

        Ok(())
    }

    // ]]] === Set ===

    // ]]] === Other ===

    /// Debugging method
    fn print_data_type(reply: &GetPropertyReply) {
        println!("Reply: {:#?}", reply);
        println!("DataType: {:#?}", AtomEnum::from(reply.type_ as u8));
    }
}

// vim: ft=rust:et:sw=4:ts=2:sts=4:tw=99:fdm=marker:fmr=[[[,]]]:
