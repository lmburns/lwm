//! The local information about the X-Server window stack

use crate::types::Window;
use std::collections::HashMap;

/// The type of layer in the [`Stack`]
#[derive(Debug)]
pub(crate) enum StackLayer {
    /// A `Desktop` layer
    Desktop,
    /// A layer that is `Below` another
    Below,
    /// A layer that is `Above` another
    Above,
    /// A `Dock` layer
    Dock,
    /// A `Notification` layer
    Notification,
    // Regular,
    // Free,
    // Transient,
    // Fullscreen,
}

/// The window `Stack` manager
#[derive(Debug)]
pub(crate) struct StackManager {
    /// All [`Window`]s mapped to their [`StackLayer`]
    window_layers: HashMap<Window, StackLayer>,

    /// Windows with a [`Desktop`](StackLayer::Desktop) layer
    desktop_windows:      Vec<Window>,
    /// Windows with a [`Below`](StackLayer::Below) layer
    below_windows:        Vec<Window>,
    /// Windows with an [`Above`](StackLayer::Above) layer
    above_windows:        Vec<Window>,
    /// Windows with a [`Dock`](StackLayer::Dock) layer
    dock_windows:         Vec<Window>,
    /// Windows with a [`Notification`](StackLayer::Notification) layer
    notification_windows: Vec<Window>,

    /// Windows to be stacked above others
    above_other: HashMap<Window, Window>,
    /// Windows to be stacked below others
    below_other: HashMap<Window, Window>,
}

impl Default for StackManager {
    fn default() -> Self {
        Self {
            window_layers:        HashMap::with_capacity(5),
            desktop_windows:      Vec::with_capacity(12),
            below_windows:        Vec::with_capacity(12),
            above_windows:        Vec::with_capacity(12),
            dock_windows:         Vec::with_capacity(12),
            notification_windows: Vec::with_capacity(12),
            above_other:          HashMap::with_capacity(25),
            below_other:          HashMap::with_capacity(25),
        }
    }
}

impl StackManager {
    /// Create a new [`StackManager`]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    // ========================== Accessor ==========================

    /// Retrive the `above_other` windows
    pub(crate) const fn above_other(&self) -> &HashMap<Window, Window> {
        &self.above_other
    }

    /// Retrive the `above_other` windows
    pub(crate) const fn below_other(&self) -> &HashMap<Window, Window> {
        &self.below_other
    }
}
