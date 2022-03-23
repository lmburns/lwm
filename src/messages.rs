//! Handle events and messages

use anyhow::{Context, Result};

/// The domain in which the messages are taking place
#[derive(Debug)]
enum Domain {
    /// Happening at the highest level
    Tree,
    /// Happening at the [`Monitor`] level
    Monitor,
    /// Happening at the [`Desktop`] level
    Desktop,
    /// Happening at the [`Node`] level
    Node
}
