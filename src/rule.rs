//! TODO:

#![allow(clippy::missing_docs_in_private_items)]

use crate::{
    monitor::client::ClientState,
    geometry::Rectangle,
    stack::StackLayer,
    core::{Direction, EventQueue, Window},
};

use anyhow::{Context, Result};
use attr_rs::{attr_accessor, attr_reader};
use serde::{Deserialize, Serialize};

// =============================== Rule ===============================

// #[derive(Debug, Clone)]
// pub(crate) struct Rule {
//     class_name:    String,
//     instance_name: String,
//     name:          String,
//     effect:        String,
//     one_shot:      bool,
//     prev:          Box<Self>,
//     next:          Box<Self>,
// }

/// A rule for a given [`Client`]
#[derive(Serialize, Deserialize, Debug, Default)]
#[attr_accessor(temp, one_shot)]
#[attr_reader(class, instance, name)]
pub(crate) struct Rule {
    class:    Option<String>,
    instance: Option<String>,
    name:     Option<String>,
    floating: Option<bool>,
    size:     Option<(u16, u16)>,
    pos:      Option<(i16, i16)>,
    effect:   String,
    one_shot: bool,
    temp:     bool,
}

impl Rule {
    /// Create a new [`Rule`]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Modify the [`Rule`]'s class
    pub(crate) fn set_class(&mut self, class: String) {
        self.class.replace(class);
    }

    pub(crate) fn set_instance(&mut self, instance: String) {
        self.instance.replace(instance);
    }

    pub(crate) fn set_name(&mut self, name: String) {
        self.name.replace(name);
    }

    pub(crate) fn set_floating(&mut self, floating: bool) {
        self.floating.replace(floating);
    }

    pub(crate) fn set_size(&mut self, size: (u16, u16)) {
        self.size.replace(size);
    }

    pub(crate) fn set_pos(&mut self, pos: (i16, i16)) {
        self.pos.replace(pos);
    }

    // pub(crate) fn apply(&self, args: &mut ClientArgs) -> bool {
    //     if let Some(floating) = self.floating {
    //         args.flags.floating = floating;
    //     }
    //     if let Some(size) = self.size {
    //         args.size = Some(size);
    //     }
    //     if let Some(pos) = self.pos {
    //         args.pos.replace(pos);
    //     }
    //     self.temp
    // }
}

mod tests {
    use super::Rule;

    #[test]
    fn fff() {
        let mut rule = Rule::new();
        rule.set_class("new".to_owned());
    }
}

// ========================= RuleConsequence ==========================

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
