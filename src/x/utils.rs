//! Various utilities specifically dealing with X

use crate::error::Error;
use anyhow::Result;

use x11rb::{rust_connection::RustConnection, wrapper::ConnectionExt as _};

// ============================= XUtility =============================

/// Wrapper to do basic X11 commands
pub(crate) struct XUtility;

impl XUtility {
    /// Setup the X11 [`Connection`](RustConnection)
    pub(crate) fn setup_connection() -> Result<(RustConnection, usize), Error> {
        RustConnection::connect(None).map_err(Error::Connection)
    }
}

// ============================== Stack ===============================

/// An element in the [`Stack`]
#[derive(Debug)]
pub(crate) struct StackElem<T> {
    /// Item in the [`Stack`]
    element: T,
    /// Item to the right of the current element
    next:    Option<usize>,
    /// Item to the left of the current element
    prev:    Option<usize>,
}

impl<T> StackElem<T> {
    /// Create a new [`StackElem`]
    const fn new(element: T) -> Self {
        Self {
            element,
            next: None,
            prev: None,
        }
    }
}

/// A stack of elements
#[derive(Default, Debug)]
pub(crate) struct Stack<T> {
    /// Elements in the stack
    elements: Vec<StackElem<T>>,
    /// Index of elements freed from the stack
    free:     Vec<usize>,
    /// Element at the head of the stack
    head:     Option<usize>,
    /// Element at the tail of the stack
    tail:     Option<usize>,
}

impl<T> Stack<T> {
    /// Get the length of the [`Stack`]
    pub(crate) fn len(&self) -> usize {
        self.elements.len() - self.free.len()
    }

    /// Get the element at the front of the [`Stack`]
    pub(crate) fn front(&self) -> Option<&T> {
        self.head.map(|x| &self.elements[x].element)
    }

    /// Get the element at the back of the [`Stack`]
    pub(crate) fn back(&self) -> Option<&T> {
        self.tail.map(|x| &self.elements[x].element)
    }

    /// Push an element to the front of the [`Stack`]
    pub(crate) fn push_front(&mut self, element: T) -> usize {
        let idx = if let Some(idx) = self.free.pop() {
            self.elements[idx].element = element;
            idx
        } else {
            self.elements.push(StackElem::new(element));
            self.elements.len() - 1
        };
        self.elements[idx].next = self.head;
        self.elements[idx].prev = None;

        match self.head {
            None => self.tail = Some(idx),
            Some(head_idx) => self.elements[head_idx].prev = Some(idx),
        }

        self.head = Some(idx);
        idx
    }

    /// Push an element to the back of the [`Stack`]
    pub(crate) fn push_back(&mut self, element: T) -> usize {
        let idx = if let Some(idx) = self.free.pop() {
            self.elements[idx].element = element;
            idx
        } else {
            self.elements.push(StackElem::new(element));
            self.elements.len() - 1
        };
        self.elements[idx].prev = self.tail;
        self.elements[idx].next = None;

        match self.tail {
            None => self.head = Some(idx),
            Some(tail_idx) => self.elements[tail_idx].next = Some(idx),
        }

        self.tail = Some(idx);
        idx
    }

    /// Remove an element from the [`Stack`]
    pub(crate) fn remove_node(&mut self, idx: usize) {
        match self.elements[idx].prev {
            Some(prev_idx) => self.elements[prev_idx].next = self.elements[idx].next,
            None => self.head = self.elements[idx].next,
        }
        match self.elements[idx].next {
            Some(next_idx) => self.elements[next_idx].prev = self.elements[idx].prev,
            None => self.tail = self.elements[idx].prev,
        }
        self.free.push(idx);
    }

    /// Return an iterator over the [`Stack`]
    pub(crate) const fn iter(&self) -> StackIter<T> {
        StackIter { stack: self, curr: self.head }
    }
}

/// An iterator over the [`Stack`]
pub(crate) struct StackIter<'a, T> {
    /// A reference to the stack
    stack: &'a Stack<T>,
    /// The current element in the iterator
    curr:  Option<usize>,
}

impl<'a, T> Iterator for StackIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        if let Some(element) = self.curr.and_then(|x| self.stack.elements.get(x)) {
            self.curr = element.next;
            Some(&element.element)
        } else {
            None
        }
    }
}
