//! Query information about the [`Window`]s on the window manager

#![allow(clippy::missing_docs_in_private_items)]

use anyhow::{Context, Result};

/// Query information about the selected [`Node`]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NodeSelect {
    automatic:     Option<bool>,
    focused:       Option<bool>,
    active:        Option<bool>,
    local:         Option<bool>,
    leaf:          Option<bool>,
    window:        Option<bool>,
    tiled:         Option<bool>,
    pseudo_tiled:  Option<bool>,
    floating:      Option<bool>,
    fullscreen:    Option<bool>,
    hidden:        Option<bool>,
    sticky:        Option<bool>,
    private:       Option<bool>,
    locked:        Option<bool>,
    marked:        Option<bool>,
    urgent:        Option<bool>,
    same_class:    Option<bool>,
    descendant_of: Option<bool>,
    ancestor_of:   Option<bool>,
    below:         Option<bool>,
    normal:        Option<bool>,
    above:         Option<bool>,
    horizontal:    Option<bool>,
    vertical:      Option<bool>,
}

/// Query information about the given [`Desktop`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DesktopSelect {
    occupied:     Option<bool>,
    focused:      Option<bool>,
    active:       Option<bool>,
    urgent:       Option<bool>,
    local:        Option<bool>,
    tiled:        Option<bool>,
    monocle:      Option<bool>,
    user_tiled:   Option<bool>,
    user_monocle: Option<bool>,
}

/// Query information about the given [`Monitor`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MonitorSelect {
    occupied: Option<bool>,
    focused:  Option<bool>,
}
