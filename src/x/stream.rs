//! Interacting directly with the X11 server

use crate::{core::Window, geometry::Dimension, x::xconnection::Atoms};
use anyhow::{Context, Result};
use attr_rs::attr_reader;
use nix::poll::{poll, PollFd, PollFlags};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs,
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::{
        io::AsRawFd,
        net::{UnixListener, UnixStream},
    },
    sync::Arc,
};
use x11rb::{connection::Connection, rust_connection::RustConnection};

// ============================== Stream ============================== [[[

/// A connection to a Unix stream socket
pub(crate) struct Stream {
    /// The stream socket
    stream:  UnixStream,
    /// Length of the data received from the stream
    length:  usize,
    /// Is the stream being read from?
    reading: bool,
    /// Data that is transferred across the socket
    data:    Vec<u8>,
}

impl Stream {
    /// Create a new [`Stream`]
    pub(crate) const fn new(stream: UnixStream) -> Self {
        Self {
            stream,
            length: 0,
            reading: false,
            data: Vec::new(),
        }
    }

    /// Send data across the stream
    pub(crate) fn send<T: Serialize>(&mut self, item: &T) -> Result<bool> {
        let data = bincode::serialize(item).context("failed to serialize data")?;
        match self
            .stream
            .write_all(
                bincode::serialize(&(data.len() as u32))
                    .context("failed to serialize data length")?
                    .as_slice(),
            )
            .and(self.stream.write_all(data.as_slice()))
        {
            Ok(_) => Ok(true),
            Err(e) => {
                log::info!("{:?}", e);
                Ok(false)
            },
        }
    }

    /// Extend the read data, indicating whether more should be read
    pub(crate) fn get_bytes(&mut self) -> bool {
        let mut bytes = [0_u8; 256];
        match self.stream.read(&mut bytes) {
            Ok(0) => true,
            Ok(len) => {
                self.data.extend(&bytes[..len]);
                false
            },
            Err(e) => {
                log::info!("{:?}", e);
                e.kind() != io::ErrorKind::WouldBlock
            },
        }
    }

    /// Receive data from the stream
    pub(crate) fn recieve<T: DeserializeOwned>(&mut self) -> (bool, Option<T>) {
        let done = self.get_bytes();
        if !self.reading && self.data.len() >= 4 {
            self.length = bincode::deserialize::<u32>(self.data.drain(..4).as_ref())
                .expect("failed to deserialize data") as usize;
            self.reading = true;
        }
        if self.reading && self.data.len() >= self.length {
            self.reading = false;
            (
                done,
                Some(
                    bincode::deserialize(self.data.drain(..self.length).as_ref())
                        .expect("failed to deserialize data"),
                ),
            )
        } else {
            (done, None)
        }
    }
} // ]]] === Stream ===

// =============================== Aux ================================ [[[

/// Auxillary connection information
#[attr_reader(dpy, atoms, meta_window, screen, socket)]
pub(crate) struct Aux {
    /// The actual [`Connection`](RustConnection)
    dpy:         Arc<RustConnection>,
    /// The [`Atoms`] of the connection
    atoms:       Atoms,
    /// Generated ID
    meta_window: Window,
    /// Screen number the connection is attached to
    screen:      usize,

    // Screen size
    screen_size: Dimension,

    /// Connection to a Unix socket
    listener: UnixListener,
    /// TODO:
    streams:  Vec<Stream>,
    /// TODO:
    poll_fds: Vec<PollFd>,
    /// Name of the socket
    socket:   String,
}

impl Aux {
    /// Create a new [`Aux`]
    pub(crate) fn new(conn: RustConnection, screen_num: usize) -> Result<Self> {
        let socket = format!("/tmp/lwm-{}.sock", whoami::username());
        drop(fs::remove_file(&socket));
        let listener = UnixListener::bind(&socket).context("failed to bind socket listener")?;
        listener
            .set_nonblocking(true)
            .context("failed to set non-blocking on `UnixListener`")?;

        let poll_fds = vec![
            PollFd::new(conn.stream().as_raw_fd(), PollFlags::POLLIN),
            PollFd::new(listener.as_raw_fd(), PollFlags::POLLIN),
        ];

        let setup = conn.setup();
        let screen = setup.roots[screen_num].clone();
        let root = screen.root;

        let screen_width = screen.width_in_pixels;
        let screen_height = screen.height_in_pixels;

        let meta_window = conn.generate_id().context("failed to generate an `ID`")?;

        log::debug!("interning Atoms");
        let atoms = Atoms::new(&conn)
            .context("failed to get `Atoms`")?
            .reply()
            .context("failed to get `Atoms` reply")?;

        let aux = Self {
            dpy: Arc::new(conn),
            atoms,
            meta_window,
            screen: screen_num,
            screen_size: Dimension::new(screen_width.into(), screen_height.into()),
            listener,
            streams: vec![],
            poll_fds,
            socket,
        };

        Ok(aux)
    }

    /// Wait for file descriptor to become available
    pub(crate) fn wait_for_updates(&mut self) {
        poll(&mut self.poll_fds, -1).ok();
    }
} // ]]] === Aux ===

impl Drop for Aux {
    fn drop(&mut self) {
        fs::remove_file(&self.socket);
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.stream.shutdown(Shutdown::Both);
    }
}

impl AsRawFd for Stream {
    fn as_raw_fd(&self) -> i32 {
        self.stream.as_raw_fd()
    }
}

// vim: ft=rust:et:sw=4:ts=2:sts=4:tw=99:fdm=marker:fmr=[[[,]]]:
