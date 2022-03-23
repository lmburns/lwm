//! Information about the `fifo`

#![allow(clippy::missing_docs_in_private_items)]

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub(crate) struct SubscriberList {
    // file: Stream,
    fifo_path: String,
    field:     usize,
    count:     usize,
    prev:      Box<Self>,
    next:      Box<Self>,
}
