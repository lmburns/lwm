//! Defines a [`Ring`], which is a structure that simplifies the manipulation of
//! an order collection of [`Window`]s.
//!
//! The [`Ring`] resembles a ring-buffer that has a single element focused at
//! all times in actuality; however, the `focused` field is an [`Option`]
//! because the `root` window is not considered focused in this implementation.
//! The [`Ring`] can be rotated in both directions and the focus will be set.
//!
//! To get an item from the [`Ring`], a [`Selector`] is used. This can retrieve
//! the focused item ([`Window`]), the item at a given index, or an item
//! fulfilling a predicate.
//!
//! To insert an item into a [`Ring`], an [`InsertPoint`] is needed. This
//! specifies an item with respect to the current time focused, or a given
//! index.
//!
//! This data structure is based off of [`penrose`][1] and a lot of the
//! modifications to the module came from [`wzrd`][2]
//!
//! This project is for me to learn how window managers work
//!
//! [1]: https://github.com/sminez/penrose.git
//! [2]: https://github.com/deurzen/wzrd

// TODO: Possibly remove unwindable

use crate::{
    types::{Identify, Idx, Xid},
    utils::BuildIdHasher,
};
use anyhow::{Context, Result};
use itertools::Itertools;
use std::{
    cmp::Ordering,
    collections::{vec_deque, HashMap, VecDeque},
    fmt::{self, Debug as Debug_},
    iter::{Enumerate, FromIterator, IntoIterator},
    ops::{Index, IndexMut},
};
use tern::t;

// ============================= Direction ============================

/// Direction that modifies item in the [`Ring`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Direction {
    /// Move to the next item
    Forward,
    /// Move to the previous item
    Backward,
}

impl Direction {
    /// Reverse the direction
    pub(crate) const fn reverse(self) -> Self {
        match self {
            Self::Forward => Self::Backward,
            Self::Backward => Self::Forward,
        }
    }
}

// ========================== InsertPoint ==========================

/// Location that an element should be inserted into a [`Ring`]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum InsertPoint {
    /// Replace the currently focused element
    ///
    /// If nothing is currently focused, the item is appended
    BeforeFocused,
    /// After the currently focused element
    AfterFocused,
    /// Before a given [`Idx`]
    BeforeIndex(Idx),
    /// After a given [`Idx`]
    AfterIndex(Idx),
    /// Before the given [`Xid`]
    BeforeIdent(Xid),
    /// After the given [`Xid`]
    AfterIdent(Xid),
    /// First item in the stack
    Front,
    /// Last item in the stack
    Back,
}

// ============================= Selector =============================

// TODO: Return everything for Any

/// Used to selector an item matching the given [`Selector`]
#[derive(Clone, Copy)]
pub(crate) enum Selector<'a, T> {
    /// Any element in the target collection.
    ///
    /// Functions that return a single element: equivalent to `Focused`
    /// Functions that return a multiple elements: returns everything
    Any,
    /// The first item in the collection
    First,
    /// The last item in the collection
    Last,
    /// Focused element in the target collection
    Focused,
    /// Element at given index
    Index(Idx),
    /// Element with/containing this client ID
    Ident(Xid),
    /// First element satisfying this condition
    Condition(&'a dyn Fn(&T) -> bool),
}

impl<T> From<Xid> for Selector<'_, T> {
    fn from(id: Xid) -> Self {
        Self::Ident(id)
    }
}

impl<T> From<Idx> for Selector<'_, T> {
    fn from(index: Idx) -> Self {
        Self::Index(index)
    }
}

impl<'a, T> From<&'a dyn Fn(&T) -> bool> for Selector<'a, T> {
    fn from(f: &'a dyn Fn(&T) -> bool) -> Self {
        Self::Condition(f)
    }
}

impl<T> fmt::Debug for Selector<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => f.debug_struct("Selector::Any").finish(),
            Self::First => f.debug_struct("Selector::First").finish(),
            Self::Last => f.debug_struct("Selector::Last").finish(),
            Self::Focused => f.debug_struct("Selector::Focused").finish(),
            Self::Index(i) => f.debug_struct("Selector::Index").field("index", i).finish(),
            Self::Ident(i) => f.debug_struct("Selector::WinId").field("id", i).finish(),
            Self::Condition(func) => f
                .debug_struct("Selector::Condition")
                .field("condition", &stringify!(func))
                .finish(),
        }
    }
}

// =============================== Ring ===============================

/// Action to perform to the stack of [`Xid`]s
#[derive(Clone, Copy, PartialEq)]
enum StackAction {
    /// Insert an item onto the stack
    Insert,
    /// Remove an item from the stack
    Remove,
}

/// The [`Ring`]-buffer holding all items `T` and the currently focused item
#[derive(Debug, Clone, Default)]
pub(crate) struct Ring<T: Identify + Debug_> {
    /// Rotatable internal buffer of items
    pub(crate) elements: VecDeque<T>,
    /// Map of an window ID and its corresponding index
    pub(crate) indices:  HashMap<Xid, Idx, BuildIdHasher>,

    /// Idx of the focused window
    pub(crate) focused: Idx,

    /// Is the [`Ring`] unwindable?
    pub(crate) unwindable: bool,
    /// List of [`Xid`]'s in the current [`Ring`]
    pub(crate) stack:      VecDeque<Xid>,
}

impl<T: Identify + Debug_> Ring<T> {
    /// Maximum size of the stack
    const MAX_STACK_LEN: usize = 0x10;

    /// Create a new [`Ring`]
    pub(crate) fn new(elements: Vec<T>, unwindable: bool) -> Self {
        Self {
            indices: elements
                .iter()
                .enumerate()
                .map(|(i, e)| (e.id(), i))
                .collect(),

            // Focus is last element
            focused: Self::last_index(&elements.iter()),

            elements: elements.into(),

            unwindable,
            stack: VecDeque::with_capacity(Self::MAX_STACK_LEN),
        }
    }

    /// Return an index based off an iterator's size
    fn last_index(iter: &impl ExactSizeIterator) -> Idx {
        iter.len().ne(&0).then(|| iter.len() - 1).unwrap_or(0)
    }

    /// Return an the focused element if it is within acceptable range
    fn index_opt(&self) -> Option<Idx> {
        (self.focused < self.len()).then(|| self.focused)
    }

    /// Reset the stack and set focus to `0`
    pub(crate) fn clear(&mut self) {
        self.focused = 0;
        self.elements.clear();
        self.indices.clear();
        self.stack.clear();
    }

    /// Return then length of the stack
    pub(crate) fn len(&self) -> usize {
        self.elements.len()
    }

    /// Check whether the stack is empty
    pub(crate) fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Check if an element is contained within the [`Ring`]
    pub(crate) fn contains(&self, element: &T) -> bool {
        self.elements.contains(element)
    }

    /// Retrieve the index of the currently focused item
    pub(crate) fn focused_index(&self) -> Idx {
        self.focused
    }

    /// Try to retrieve the focused element
    pub(crate) fn focused(&self) -> Option<&T> {
        self.elements.get(self.focused)
    }

    /// Try to retrieve the focused element as mutable
    pub(crate) fn focused_mut(&mut self) -> Option<&mut T> {
        self.elements.get_mut(self.focused)
    }

    /// Return the focused element without checking existence
    pub(crate) fn focused_unchecked(&self) -> &T {
        &self.elements[self.focused]
    }

    /// Return the focused element as mutable without checking existence
    pub(crate) fn focused_mut_unchecked(&mut self) -> &mut T {
        &mut self.elements[self.focused]
    }

    /// Retrieve an element at the given [`Idx`]
    pub(crate) fn get(&self, index: Idx) -> Option<&T> {
        self.elements.get(index)
    }

    /// Retrieve an element at the given [`Idx`], return as mutable
    pub(crate) fn get_mut(&mut self, index: Idx) -> Option<&mut T> {
        self.elements.get_mut(index)
    }

    /// Retrieve an element at the given [`Idx`] without checking existence
    pub(crate) fn get_unchecked(&self, index: Idx) -> &T {
        &self.elements[index]
    }

    /// Retrieve an element at the given [`Idx`] without checking existence
    /// Return as mutable
    pub(crate) fn get_unchecked_mut(&mut self, index: Idx) -> &mut T {
        &mut self.elements[index]
    }

    /// Retrieve an element matching a [`Selector`]
    pub(crate) fn get_for(&self, sel: &Selector<T>) -> Option<&T> {
        match sel {
            Selector::Focused | Selector::Any => self.focused(),
            Selector::First => self.get(0),
            Selector::Last => self.get(Self::last_index(&self.iter())),
            Selector::Index(idx) => self.get(*idx),
            Selector::Condition(f) => self.by(f).map(|(_, e)| e),
            Selector::Ident(id) => {
                if let Some(index) = self.id_to_index(*id) {
                    return self.get_for(&Selector::Index(index));
                }

                None
            },
        }
    }

    /// Retrieve an element matching a [`Selector`], return as mutable
    pub(crate) fn get_for_mut(&mut self, sel: &Selector<T>) -> Option<&mut T> {
        match sel {
            Selector::Focused | Selector::Any => self.focused_mut(),
            Selector::First => self.get_mut(0),
            Selector::Last => self.get_mut(Self::last_index(&self.iter())),
            Selector::Index(idx) => self.get_mut(*idx),
            Selector::Condition(f) => self.by_mut(f).map(|(_, e)| e),
            Selector::Ident(id) => {
                if let Some(idx) = self.id_to_index(*id) {
                    return self.get_for_mut(&Selector::Index(idx));
                }

                None
            },
        }
    }

    /// Retrieve all elements matching a [`Selector`]
    pub(crate) fn get_all_for(&self, sel: &Selector<T>) -> Vec<&T> {
        match sel {
            Selector::Focused | Selector::Any => self.focused().into_iter().collect(),
            Selector::First => self.get(0).into_iter().collect(),
            Selector::Last => self
                .get(Self::last_index(&self.iter()))
                .into_iter()
                .collect(),
            Selector::Index(idx) => self.get(*idx).into_iter().collect(),
            Selector::Condition(f) => self.filter(|e| f(e)),
            Selector::Ident(id) => {
                if let Some(index) = self.id_to_index(*id) {
                    return self.get_all_for(&Selector::Index(index));
                }

                vec![]
            },
        }
    }

    /// Retrieve all elements matching a [`Selector`]; return as mutable
    pub(crate) fn get_all_for_mut(&mut self, sel: &Selector<T>) -> Vec<&mut T> {
        match sel {
            Selector::Focused | Selector::Any => self.focused_mut().into_iter().collect(),
            Selector::First => self.get_mut(0).into_iter().collect(),
            Selector::Last => self
                .get_mut(Self::last_index(&self.iter()))
                .into_iter()
                .collect(),
            Selector::Index(idx) => self.get_mut(*idx).into_iter().collect(),
            Selector::Condition(f) => self.filter_mut(|e| f(e)),
            Selector::Ident(id) => {
                if let Some(idx) = self.id_to_index(*id) {
                    return self.get_all_for_mut(&Selector::Index(idx));
                }

                vec![]
            },
        }
    }

    /// Check whether moving to the next item would wrap front or backwards
    pub(crate) fn will_wrap_from(&self, index: Idx, direction: Direction) -> bool {
        match direction {
            Direction::Forward =>
                if index == Self::last_index(&self.iter()) {
                    log::debug!("wrapping `Ring` forward");
                    return true;
                },
            Direction::Backward =>
                if index == 0 {
                    log::debug!("wrapping `Ring` backwards");
                    return true;
                },
        }
        false
    }

    /// Rotate the inner elements a given step size
    pub(crate) fn rotate_by(&mut self, step: Idx, direction: Direction) {
        match direction {
            Direction::Forward => {
                self.elements.rotate_right(step);
            },
            Direction::Backward => {
                self.elements.rotate_left(step);
            },
        }
    }

    /// Rotate the inner elements based on the [`Direction`]
    pub(crate) fn rotate(&mut self, direction: Direction) {
        if !self.elements.is_empty() {
            self.rotate_by(1, direction);

            self.indices.clear();

            for (i, id) in self
                .elements
                .iter()
                .enumerate()
                .map(|(idx, win)| (idx, win.id()))
            {
                self.indices.insert(id, i);
            }
        }
    }

    /// Swap element `i` and `j` matching the given [`Selector`]s
    pub(crate) fn swap(&mut self, sel1: &Selector<T>, sel2: &Selector<T>) {
        let index1 = self.index_for(sel1);

        if let Some(index1) = index1 {
            let index2 = self.index_for(sel2);

            if let Some(index2) = index2 {
                self.elements.swap(index1, index2);
            }
        }
    }

    /// Move the focused element while keeping other elements in order
    pub(crate) fn drag_focused(&mut self, dir: Direction) -> Option<&T> {
        match (self.focused, self.next_index(dir), dir) {
            (0, _, Direction::Backward) | (_, 0, Direction::Forward) => self.rotate(dir),
            (focused, next, _) => {
                let focused_id = self.get_unchecked(focused).id();
                let next_id = self.get_unchecked(next).id();

                self.elements.swap(focused, next);

                *self.indices.get_mut(&focused_id)? = next;
                *self.indices.get_mut(&next_id)? = focused;
            },
        };

        self.cycle_focused(dir)
    }

    /// Get the next index based on a given index
    fn next_index_from(&self, index: Idx, direction: Direction) -> Idx {
        let end = Self::last_index(&self.iter());

        match direction {
            Direction::Forward => t!(self.will_wrap_from(index, direction)  ? 0 : index + 1),
            Direction::Backward => t!(self.will_wrap_from(index, direction) ? end : index - 1),
        }
    }

    /// Retrieve the next item's index based on the [`Direction`]
    pub(crate) fn next_index(&self, direction: Direction) -> Idx {
        self.next_index_from(self.focused, direction)
    }

    /// Retrieve the next element based on the [`Direction`]
    pub(crate) fn next_element(&self, direction: Direction) -> Option<&T> {
        let next_index = self.next_index(direction);
        self.get_for(&Selector::Index(next_index))
    }

    /// Retrieve the index of the first item matching a [`Selector`]
    pub(crate) fn index_for(&self, sel: &Selector<T>) -> Option<Idx> {
        match sel {
            Selector::Focused | Selector::Any => Some(self.focused_index()),
            Selector::Index(idx) => {
                if *idx < self.len() {
                    return Some(*idx);
                }

                None
            },
            Selector::Ident(id) => {
                if let Some(idx) = self.id_to_index(*id) {
                    return self.index_for(&Selector::Index(idx));
                }

                None
            },
            Selector::First => Some(0),
            Selector::Last => Some(self.len() - 1),
            Selector::Condition(f) => self.by(f).map(|(i, _)| i),
        }
    }

    /// Retrieve the index and the element matching a [`Selector`]
    pub(crate) fn indexed_element(&self, s: &Selector<T>) -> Option<(Idx, &T)> {
        self.index_for(s).map(|i| (i, &self.elements[i]))
    }

    /// Remove an element at the given [`Idx`]
    pub(crate) fn remove(&mut self, index: Idx) -> Option<T> {
        self.elements.remove(index)
    }

    /// Remove the given element from the stack
    fn remove_element(&mut self, index: Idx, element: &Option<T>) {
        if let Some(element) = element {
            let id = element.id();

            self.indices.remove(&id);
            self.remove_from_stack(id);
            self.sync_indices(index, StackAction::Remove);
        }
    }

    // TEST: done
    /// Activate (i.e., focus) the item matching the [`Selector`]
    pub(crate) fn focus_for(&mut self, sel: &Selector<T>) -> Option<&T> {
        match sel {
            Selector::Focused | Selector::Any => self.focused(),
            Selector::Index(index) => {
                self.push_focused_to_stack();
                self.focused = *index;
                self.focused()
            },
            Selector::Ident(id) => {
                if let Some(idx) = self.id_to_index(*id) {
                    return self.focus_for(&Selector::Index(idx));
                }

                None
            },
            Selector::First => {
                self.push_focused_to_stack();
                self.focused = 0;
                self.focused()
            },
            Selector::Last => {
                self.push_focused_to_stack();
                self.focused = Self::last_index(&self.iter());
                self.focused()
            },
            Selector::Condition(f) =>
                if let Some((index, _)) = self.by(f) {
                    self.push_focused_to_stack();
                    self.focused = index;
                    Some(self.focused_unchecked())
                } else {
                    None
                },
        }
    }

    // TEST: done
    /// Remove an element matching the given [`Selector`]
    pub(crate) fn remove_for(&mut self, sel: &Selector<T>) -> Option<T> {
        let (index, element) = match sel {
            Selector::Focused | Selector::Any => (self.focused, self.remove(self.focused)),
            Selector::Index(idx) => (*idx, self.remove(*idx)),
            Selector::Ident(id) => {
                if let Some(idx) = self.id_to_index(*id) {
                    return self.remove_for(&Selector::Index(idx));
                }

                return None;
            },
            Selector::First => (0, self.remove(0)),
            Selector::Last => {
                let end = Self::last_index(&self.iter());
                (end, self.remove(end))
            },
            Selector::Condition(f) =>
                if let Some((idx, _)) = self.by(f) {
                    (idx, self.remove(idx))
                } else {
                    return None;
                },
        };

        self.remove_element(index, &element);
        element
    }

    /// Insert an element at the given [`Idx`]
    pub(crate) fn insert(&mut self, index: Idx, element: T) {
        self.push_focused_to_stack();
        self.sync_indices(index, StackAction::Insert);
        self.indices.insert(element.id(), index);
        self.elements.insert(index, element);
        self.focused = index;
    }

    /// Insert an element at the given [`InsertPoint`]
    pub(crate) fn insert_at(&mut self, insert_pos: &InsertPoint, element: T) {
        match insert_pos {
            InsertPoint::Front => self.push_front(element),
            InsertPoint::Back => self.push_back(element),
            InsertPoint::BeforeFocused => self.insert(self.focused_index(), element),
            InsertPoint::AfterFocused =>
                self.insert_at(&InsertPoint::AfterIndex(self.focused_index()), element),
            InsertPoint::BeforeIndex(index) => self.insert(*index, element),
            InsertPoint::AfterIndex(index) => {
                let next_index = index + 1;

                if next_index > self.elements.len() {
                    self.push_back(element);
                } else {
                    self.insert(next_index, element);
                }
            },
            InsertPoint::BeforeIdent(id) =>
                if let Some(index) = self.id_to_index(*id) {
                    self.insert_at(&InsertPoint::BeforeIndex(index), element);
                },
            InsertPoint::AfterIdent(id) =>
                if let Some(index) = self.id_to_index(*id) {
                    self.insert_at(&InsertPoint::AfterIndex(index), element);
                },
        }
    }

    /// Modify `indices` and focused element based on [`StackAction`]
    fn sync_indices(&mut self, pivot_index: Idx, action: StackAction) -> Result<()> {
        for index in pivot_index..self.len() {
            let id = self
                .get(index)
                .context(format!("failed to get element with index {}", index))?
                .id();

            match action {
                StackAction::Remove =>
                    *self
                        .indices
                        .get_mut(&id)
                        .context(format!("failed to get mutable indice with id: {}", id))? -= 1,
                StackAction::Insert =>
                    *self
                        .indices
                        .get_mut(&id)
                        .context(format!("failed to get mutable indice with id: {}", id))? += 1,
            }
        }

        if action == StackAction::Remove {
            match pivot_index.cmp(&self.focused) {
                Ordering::Equal => {
                    if let Some(id) = self.pop_from_stack() {
                        if let Some(idx) = self.id_to_index(id) {
                            self.focused = idx;
                            return Ok(());
                        }
                    }

                    self.focused = Self::last_index(&self.iter());
                },
                Ordering::Less =>
                    if self.focused > 0 {
                        self.focused -= 1;
                    },
                Ordering::Greater => {},
            }
        }

        Ok(())
    }

    // ============================== Stack ===============================

    /// Return the inner stack of window IDs
    pub(crate) fn stack(&self) -> &VecDeque<Xid> {
        &self.stack
    }

    /// Prepend the given element to the beginning of elements
    /// Focus follows this element
    pub(crate) fn push_front(&mut self, element: T) {
        log::debug!("pushing {:?} to front of elements", element);
        self.push_focused_to_stack();
        self.sync_indices(0, StackAction::Insert);
        self.indices.insert(element.id(), 0);
        self.elements.push_front(element);
        self.focused = 0;
    }

    // TEST: Focus follows this element
    /// Append the given element to the end of elements
    /// Focus stays on previous last element
    pub(crate) fn push_back(&mut self, element: T) {
        log::debug!("pushing {:?} to back of elements", element);
        let end = self.len();

        self.push_focused_to_stack();
        self.indices.insert(element.id(), end);
        self.elements.push_back(element);
        self.focused = end;
    }

    /// Move the focus through the elements. All items retain their position
    ///
    /// [`drag_focused`](#method.drag_focused) moves the *same* focused element
    /// through other elements, while they keep their position
    pub(crate) fn cycle_focused(&mut self, direction: Direction) -> Option<&T> {
        self.push_focused_to_stack();
        self.focused = self.next_index(direction);
        self.focused()
    }

    /// If the currently focused element is found on the stack, it is removed
    /// and then added back
    pub(crate) fn stack_after_focus(&self) -> Vec<Xid> {
        let mut stack: Vec<Xid> = self.stack.iter().copied().collect();

        if let Some(index) = self.index_opt() {
            if let Some(id) = self.index_to_id(index) {
                if let Some(found_index) = stack.iter().rposition(|i| *i == id) {
                    stack.remove(found_index);
                }

                stack.push(id);
            }
        }

        stack
    }

    /// Push the [`Xid`] matching the index to the back of the stack
    fn push_index_to_stack(&mut self, index: Option<Idx>) {
        if self.unwindable {
            if let Some(index) = index {
                if let Some(id) = self.index_to_id(index) {
                    self.remove_from_stack(id);
                    self.stack.push_back(id);
                }
            }
        }
    }

    /// Push the currently focused [`Xid`] to the back of the stack
    fn push_focused_to_stack(&mut self) {
        if self.unwindable {
            self.push_index_to_stack(self.index_opt());
        }
    }

    /// Remove the item from the stack matching the [`Xid`]
    fn remove_from_stack(&mut self, id: Xid) {
        if self.unwindable {
            if let Some(found_index) = self.stack.iter().rposition(|i| *i == id) {
                self.stack.remove(found_index);
            }
        }
    }

    /// Remove last item from the `stack`
    fn pop_from_stack(&mut self) -> Option<Xid> {
        if !self.unwindable {
            return None;
        }

        self.stack.pop_back()
    }

    // =================== Combinator/Helper Functions ====================

    /// Return an iterator over the elements
    pub(crate) fn iter(&self) -> vec_deque::Iter<'_, T> {
        self.elements.iter()
    }

    /// Return a mutable iterator over the elements
    pub(crate) fn iter_mut(&mut self) -> vec_deque::IterMut<'_, T> {
        self.elements.iter_mut()
    }

    /// Return an enumerated iterator over the elements
    pub(crate) fn enumerate(&self) -> Vec<(Idx, &T)> {
        self.iter().enumerate().collect()
    }

    /// Apply a function to each element, returning the modified vector
    pub(crate) fn map<F: FnMut(&T) -> U, U>(&self, f: F) -> Vec<U> {
        self.iter().map(f).collect()
    }

    /// Apply a function to the focused element
    pub(crate) fn on_focused<F: Fn(&T)>(&self, f: F) {
        self.focused().map_or_else(
            || log::debug!("function called on non-existent focused element"),
            f,
        );
    }

    /// Apply a function to the focused (mutable) element
    pub(crate) fn on_focused_mut<F: FnMut(&mut T)>(&mut self, mut f: F) {
        self.focused_mut().map_or_else(
            || log::debug!("function called on non-existent focused element"),
            f,
        );
    }

    // Is this and on_focused ever needed?

    /// Apply a function to the first element matching a [`Selector`]
    pub(crate) fn apply_for<F: Fn(&T)>(&self, sel: &Selector<T>, f: F) {
        if let Some(index) = self.index_for(sel) {
            f(self.get_unchecked(index));
        }
    }

    /// Apply a function to the first element matching a [`Selector`] (mutably)
    pub(crate) fn apply_for_mut<F: FnMut(&mut T)>(&mut self, sel: &Selector<T>, mut f: F) {
        if let Some(index) = self.index_for(sel) {
            f(self.get_unchecked_mut(index));
        }
    }

    /// Apply a function to all elements
    pub(crate) fn on_all<F: Fn(&T)>(&self, f: F) {
        self.iter().for_each(f);
    }

    /// Apply a function to all elements (mutably)
    pub(crate) fn on_all_mut<F: FnMut(&mut T)>(&mut self, mut f: F) {
        self.iter_mut().for_each(f);
    }

    /// Apply a function to all elements matching a [`Selector`]
    pub(crate) fn on_all_for<F: Fn(&T)>(&self, f: F, sel: &Selector<T>) {
        for element in self.get_all_for(sel) {
            f(element);
        }
    }

    /// Apply a function to all elements matching a [`Selector`] (mutably)
    pub(crate) fn on_all_for_mut<F: FnMut(&mut T)>(&mut self, mut f: F, sel: &Selector<T>) {
        for element in self.get_all_for_mut(sel) {
            f(element);
        }
    }

    /// Wrapper function to filter the inner elements
    fn filter<F>(&self, mut f: F) -> Vec<&T>
    where
        F: FnMut(&T) -> bool,
    {
        self.iter().filter(|x| f(x)).collect()
    }

    /// Wrapper function to filter the inner elements; return as mutable
    fn filter_mut<F>(&mut self, mut f: F) -> Vec<&mut T>
    where
        F: FnMut(&T) -> bool,
    {
        self.iter_mut().filter(|x| f(x)).collect()
    }

    /// Search the elements for one matching a condition.
    /// Returns the matched element and its index, if any
    fn by<F>(&self, f: F) -> Option<(Idx, &T)>
    where
        F: Fn(&T) -> bool,
    {
        self.iter().enumerate().find(|(_, e)| f(*e))
    }

    /// Search the elements for one matching a condition.
    /// Returns the matched element and its index, if any, as mutable
    fn by_mut<F>(&mut self, f: F) -> Option<(Idx, &mut T)>
    where
        F: Fn(&T) -> bool,
    {
        self.iter_mut().enumerate().find(|(_, e)| f(*e))
    }

    /// Get the index of the element
    fn index_of(&self, element: &T) -> Option<Idx> {
        self.id_to_index(element.id())
    }

    /// Convert an index to a window identifier
    fn index_to_id(&self, index: Idx) -> Option<Xid> {
        if let Some(element) = self.get(index) {
            return Some(element.id());
        }

        None
    }

    /// Convert a window identifier into an index
    fn id_to_index(&self, id: Xid) -> Option<Idx> {
        if let Some(index) = self.indices.get(&id) {
            return Some(*index);
        }

        None
    }
}

impl<T: PartialEq + Identify + Debug_> Ring<T> {
    /// Test two [`Selector`]s equivalency
    pub(crate) fn cmp_selectors(&self, s: &Selector<T>, t: &Selector<T>) -> bool {
        if let (Some(e), Some(f)) = (self.index_for(s), self.index_for(t)) {
            e == f
        } else {
            false
        }
    }
}

impl<T: Clone + Identify + Debug_> Ring<T> {
    /// Allow the cloning of the inner elements
    pub(crate) fn as_vec(&self) -> Vec<T> {
        self.iter().cloned().collect()
    }
}

impl<T: Identify + Debug_> Index<Idx> for Ring<T> {
    type Output = T;

    /// [`Index`] the inner elements
    fn index(&self, index: Idx) -> &Self::Output {
        self.get_unchecked(index)
    }
}

impl<T: Identify + Debug_> IndexMut<Idx> for Ring<T> {
    /// [`Index`] the inner elements with mutability
    fn index_mut(&mut self, index: Idx) -> &mut Self::Output {
        self.get_unchecked_mut(index)
    }
}

// TODO:
// ========================== May not be needed ==========================

impl<T: Identify + Debug_> FromIterator<T> for Ring<T> {
    /// Create a new [`Ring`] from an iterator
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut ring = Self::new(vec![], true);
        for element in iter {
            ring.push_back(element);
        }

        ring
    }
}

impl<T: Identify + Debug_> IntoIterator for Ring<T> {
    type IntoIter = vec_deque::IntoIter<T>;
    type Item = T;

    /// Consumes [`VecDeque`] into a front-to-back iterator
    /// Yields elements by value
    fn into_iter(self) -> vec_deque::IntoIter<T> {
        self.elements.into_iter()
    }
}

impl<'a, T: Identify + Debug_> IntoIterator for &'a Ring<T> {
    type IntoIter = vec_deque::Iter<'a, T>;
    type Item = &'a T;

    fn into_iter(self) -> vec_deque::Iter<'a, T> {
        self.iter()
    }
}

impl<'a, T: Identify + Debug_> IntoIterator for &'a mut Ring<T> {
    type IntoIter = vec_deque::IterMut<'a, T>;
    type Item = &'a mut T;

    fn into_iter(self) -> vec_deque::IterMut<'a, T> {
        self.iter_mut()
    }
}

/// Tests for [`Ring`]
mod tests {
    use super::{Direction, Identify, InsertPoint, Ring, Selector, Xid};

    /// Implement [`Identify`] for tests
    impl Identify for i32 {
        fn id(&self) -> Xid {
            *self as Xid
        }
    }

    impl Identify for &str {
        fn id(&self) -> Xid {
            self.len() as Xid
        }
    }

    /// Shorthand way of testing two arrays
    macro_rules! test_focus {
        ($ring:ident, [$($element:expr),+] => [$($against:expr),+]) => {
            assert_eq!(
                ($($ring.indices.get(&$element)),+), ($($against),+)
            );
        };
    }

    #[test]
    fn remove_element_before_focus() {
        let mut ring = Ring::new(
            vec![0_i32, 10_i32, 20_i32, 30_i32, 40_i32, 50_i32, 60_i32],
            false,
        );

        // Last element is focused
        assert_eq!(ring.focused, 6);

        // Remove the element at index 2 (i.e., 20)
        let popped = ring.remove_for(&Selector::Index(2));
        assert_eq!(popped, Some(20_i32));
        assert_eq!(ring.focused, 5);
        test_focus!(
            ring,
               [0,        10,       20,   30,       40,      50,        60]
            => [Some(&0), Some(&1), None, Some(&2), Some(&3), Some(&4), Some(&5)]
        );

        // Remove the element at index 2 (i.e., 30)
        let popped = ring.remove_for(&Selector::Index(2));
        assert_eq!(popped, Some(30_i32));
        assert_eq!(ring.focused, 4);
        test_focus!(
            ring,
               [0,        10,       20,   30,   40,       50,       60]
            => [Some(&0), Some(&1), None, None, Some(&2), Some(&3), Some(&4)]
        );

        let popped = ring.remove_for(&Selector::Index(2));
        assert_eq!(popped, Some(40_i32));
        assert_eq!(ring.focused, 3);
        test_focus!(
            ring,
               [0,        10,       20,   30,   40,   50,       60]
            => [Some(&0), Some(&1), None, None, None, Some(&2), Some(&3)]
        );

        let popped = ring.remove_for(&Selector::Index(2));
        assert_eq!(popped, Some(50_i32));
        assert_eq!(ring.focused, 2);
        test_focus!(
            ring,
               [0,        10,       20,   30,   40,   50,   60]
            => [Some(&0), Some(&1), None, None, None, None, Some(&2)]
        );

        let popped = ring.remove_for(&Selector::Index(2));
        assert_eq!(popped, Some(60_i32));
        assert_eq!(ring.focused, 1);
        test_focus!(
            ring,
               [0,        10,       20,   30,   40,   50,   60]
            => [Some(&0), Some(&1), None, None, None, None, None]
        );

        let popped = ring.remove_for(&Selector::Index(1));
        assert_eq!(popped, Some(10_i32));
        assert_eq!(ring.focused, 0);
        test_focus!(
            ring,
               [0,        10,   20,   30,   40,   50,   60]
            => [Some(&0), None, None, None, None, None, None]
        );

        let popped = ring.remove_for(&Selector::Index(0));
        assert_eq!(popped, Some(0_i32));
        assert_eq!(ring.focused, 0);
        test_focus!(
            ring,
               [0,    10,   20,   30,   40,   50,   60]
            => [None, None, None, None, None, None, None]
        );
    }

    #[test]
    fn removing_last_element_at_focus() {
        let mut ring = Ring::new(
            vec![0_i32, 10_i32, 20_i32, 30_i32, 40_i32, 50_i32, 60_i32],
            false,
        );

        assert_eq!(ring.focused, 6);

        ring.remove_for(&Selector::Index(6));
        assert_eq!(ring.focused, 5);
        test_focus!(
            ring,
               [0,        10,       20,       30,       40,       50,       60]
            => [Some(&0), Some(&1), Some(&2), Some(&3), Some(&4), Some(&5), None]
        );

        ring.remove_for(&Selector::Index(5));
        assert_eq!(ring.focused, 4);
        test_focus!(
            ring,
               [0,        10,       20,       30,       40,       50,   60]
            => [Some(&0), Some(&1), Some(&2), Some(&3), Some(&4), None, None]
        );

        ring.remove_for(&Selector::Index(4));
        assert_eq!(ring.focused, 3);
        test_focus!(
            ring,
               [0,        10,       20,       30,       40,   50,   60]
            => [Some(&0), Some(&1), Some(&2), Some(&3), None, None, None]
        );

        // Duplicates do nothing
        ring.remove_for(&Selector::Index(4));
        assert_eq!(ring.focused, 3);
        test_focus!(
            ring,
               [0,        10,       20,       30,       40,   50,   60]
            => [Some(&0), Some(&1), Some(&2), Some(&3), None, None, None]
        );

        ring.remove_for(&Selector::Index(3));
        assert_eq!(ring.focused, 2);
        test_focus!(
            ring,
               [0,        10,       20,       30,   40,   50,   60]
            => [Some(&0), Some(&1), Some(&2), None, None, None, None]
        );

        ring.remove_for(&Selector::Index(2));
        assert_eq!(ring.focused, 1);
        test_focus!(
            ring,
               [0,        10,       20,   30,   40,   50,   60]
            => [Some(&0), Some(&1), None, None, None, None, None]
        );

        ring.remove_for(&Selector::Index(1));
        assert_eq!(ring.focused, 0);
        test_focus!(
            ring,
               [0,        10,   20,   30,   40,   50,   60]
            => [Some(&0), None, None, None, None, None, None]
        );

        ring.remove_for(&Selector::Index(0));
        assert_eq!(ring.focused, 0);
        test_focus!(
            ring,
               [0,    10,   20,   30,   40,   50,   60]
            => [None, None, None, None, None, None, None]
        );

        // Duplicates do nothing
        ring.remove_for(&Selector::Index(0));
        assert_eq!(ring.focused, 0);
        test_focus!(
            ring,
               [0,    10,   20,   30,   40,   50,   60]
            => [None, None, None, None, None, None, None]
        );
    }

    #[test]
    fn removing_first_element_at_focus() {
        let mut ring = Ring::new(
            vec![0_i32, 10_i32, 20_i32, 30_i32, 40_i32, 50_i32, 60_i32],
            false,
        );

        assert_eq!(ring.focused, 6);
        ring.focus_for(&Selector::Index(0));
        assert_eq!(ring.focused, 0);

        ring.remove_for(&Selector::Index(0));
        assert_eq!(ring.focused, 5);
        test_focus!(
            ring,
               [0,    10,       20,       30,       40,       50,       60]
            => [None, Some(&0), Some(&1), Some(&2), Some(&3), Some(&4), Some(&5)]
        );

        ring.focus_for(&Selector::Index(0));
        assert_eq!(ring.focused, 0);

        ring.remove_for(&Selector::Index(0));
        assert_eq!(ring.focused, 4);
        test_focus!(
            ring,
               [0,    10,   20,       30,       40,       50,       60]
            => [None, None, Some(&0), Some(&1), Some(&2), Some(&3), Some(&4)]
        );

        ring.focus_for(&Selector::Index(0));
        assert_eq!(ring.focused, 0);

        ring.remove_for(&Selector::Index(0));
        assert_eq!(ring.focused, 3);
        test_focus!(
            ring,
               [0,    10,   20,   30,       40,       50,       60]
            => [None, None, None, Some(&0), Some(&1), Some(&2), Some(&3)]
        );

        ring.focus_for(&Selector::Index(0));
        assert_eq!(ring.focused, 0);

        ring.remove_for(&Selector::Index(0));
        assert_eq!(ring.focused, 2);
        test_focus!(
            ring,
               [0,    10,   20,   30,   40,       50,       60]
            => [None, None, None, None, Some(&0), Some(&1), Some(&2)]
        );

        ring.focus_for(&Selector::Index(0));
        assert_eq!(ring.focused, 0);

        ring.remove_for(&Selector::Index(0));
        assert_eq!(ring.focused, 1);
        test_focus!(
            ring,
               [0,    10,   20,   30,   40,   50,       60]
            => [None, None, None, None, None, Some(&0), Some(&1)]
        );

        ring.focus_for(&Selector::Index(0));
        assert_eq!(ring.focused, 0);

        ring.remove_for(&Selector::Index(0));
        assert_eq!(ring.focused, 0);
        test_focus!(
            ring,
               [0,    10,   20,   30,   40,   50,   60]
            => [None, None, None, None, None, None, Some(&0)]
        );

        ring.focus_for(&Selector::Index(0));
        assert_eq!(ring.focused, 0);

        ring.remove_for(&Selector::Index(0));
        assert_eq!(ring.focused, 0);
        test_focus!(
            ring,
               [0,    10,   20,   30,   40,   50,   60]
            => [None, None, None, None, None, None, None]
        );

        ring.focus_for(&Selector::Index(0));
        assert_eq!(ring.focused, 0);

        ring.remove_for(&Selector::Index(0));
        assert_eq!(ring.focused, 0);
        test_focus!(
            ring,
               [0,    10,   20,   30,   40,   50,   60]
            => [None, None, None, None, None, None, None]
        );
    }

    #[test]
    fn focus_for_index() {
        let mut ring = Ring::new(vec![1_i32, 2_i32, 3_i32], false);
        assert_eq!(ring.focused(), Some(&3_i32));

        ring.focus_for(&Selector::Index(0));
        assert_eq!(ring.focused(), Some(&1_i32));

        ring.rotate(Direction::Forward);
        ring.focus_for(&Selector::Index(1));
        assert_eq!(ring.focused(), Some(&1_i32));

        // Focus stays on first item
        ring.rotate(Direction::Forward);
        assert_eq!(ring.focused(), Some(&3_i32));

        ring.focus_for(&Selector::First);
        assert_eq!(ring.focused(), Some(&2_i32));

        ring.focus_for(&Selector::Last);
        assert_eq!(ring.focused(), Some(&1_i32));
    }

    #[test]
    fn focus_for_condition() {
        let mut ring = Ring::new(vec![1_i32, 2_i32, 3_i32, 4_i32, 5_i32, 6_i32], false);
        assert_eq!(ring.focused, 5);
        assert_eq!(
            ring.focus_for(&Selector::Condition(&|e| e % 2_i32 == 0_i32)),
            Some(&2_i32)
        );
        assert_eq!(
            ring.focus_for(&Selector::Condition(&|e| e % 7_i32 == 0_i32)),
            None
        );
    }

    #[test]
    fn cycle_focused() {
        let mut ring = Ring::new(vec![1_i32, 2_i32, 3_i32], false);
        assert_eq!(ring.focused, 2);

        assert_eq!(ring.cycle_focused(Direction::Forward), Some(&1_i32));
        assert_eq!(ring.focused, 0);
        assert_eq!(ring.as_vec(), vec![1_i32, 2_i32, 3_i32]);

        assert_eq!(ring.cycle_focused(Direction::Backward), Some(&3_i32));
        assert_eq!(ring.as_vec(), vec![1_i32, 2_i32, 3_i32]);
    }

    #[test]
    fn rotating_focus_stays_on_last_element() {
        let mut ring = Ring::new(vec![1_i32, 2_i32, 3_i32], false);

        // Last element is focused each time
        assert_eq!(ring.focused(), Some(&3_i32));

        ring.rotate(Direction::Forward);
        assert_eq!(ring.as_vec(), vec![3_i32, 1_i32, 2_i32]);
        assert_eq!(ring.focused(), Some(&2_i32));

        ring.rotate(Direction::Forward);
        assert_eq!(ring.as_vec(), vec![2_i32, 3_i32, 1_i32]);
        assert_eq!(ring.focused(), Some(&1_i32));

        ring.rotate(Direction::Backward);
        assert_eq!(ring.as_vec(), vec![3_i32, 1_i32, 2_i32]);
        assert_eq!(ring.focused(), Some(&2_i32));
    }

    #[test]
    fn drag_element_forward() {
        let mut ring = Ring::new(vec![1_i32, 2_i32, 3_i32, 4_i32], false);
        assert_eq!(ring.focused(), Some(&4_i32));

        assert_eq!(ring.drag_focused(Direction::Forward), Some(&4_i32));
        assert_eq!(ring.elements, vec![4_i32, 1_i32, 2_i32, 3_i32]);

        assert_eq!(ring.drag_focused(Direction::Forward), Some(&4_i32));
        assert_eq!(ring.elements, vec![1_i32, 4_i32, 2_i32, 3_i32]);

        assert_eq!(ring.drag_focused(Direction::Forward), Some(&4_i32));
        assert_eq!(ring.elements, vec![1_i32, 2_i32, 4_i32, 3_i32]);
        //
        assert_eq!(ring.drag_focused(Direction::Forward), Some(&4_i32));
        assert_eq!(ring.elements, vec![1_i32, 2_i32, 3_i32, 4_i32]);

        assert_eq!(ring.focused(), Some(&4_i32));
    }

    #[test]
    fn drag_element_backward() {
        let mut ring = Ring::new(vec![1_i32, 2_i32, 3_i32, 4_i32], false);
        assert_eq!(ring.focused(), Some(&4_i32));

        assert_eq!(ring.drag_focused(Direction::Backward), Some(&4_i32));
        assert_eq!(ring.elements, vec![1_i32, 2_i32, 4_i32, 3_i32]);

        assert_eq!(ring.drag_focused(Direction::Backward), Some(&4_i32));
        assert_eq!(ring.elements, vec![1_i32, 4_i32, 2_i32, 3_i32]);

        assert_eq!(ring.drag_focused(Direction::Backward), Some(&4_i32));
        assert_eq!(ring.elements, vec![4_i32, 1_i32, 2_i32, 3_i32]);
        //
        assert_eq!(ring.drag_focused(Direction::Backward), Some(&4_i32));
        assert_eq!(ring.elements, vec![1_i32, 2_i32, 3_i32, 4_i32]);

        assert_eq!(ring.focused(), Some(&4_i32));
    }

    #[test]
    fn remove_focused() {
        let mut ring = Ring::new(vec![1_i32, 2_i32, 3_i32], false);
        ring.focused = 2;
        assert_eq!(ring.focused(), Some(&3_i32));

        assert_eq!(ring.remove_for(&Selector::Focused), Some(3_i32));
        assert_eq!(ring.focused_index(), 1);
        assert_eq!(ring.focused(), Some(&2_i32));

        assert_eq!(ring.remove_for(&Selector::Focused), Some(2_i32));
        assert_eq!(ring.focused(), Some(&1_i32));

        assert_eq!(ring.remove_for(&Selector::Focused), Some(1_i32));
        assert_eq!(ring.focused(), None);

        assert_eq!(ring.remove_for(&Selector::Focused), None);
    }

    #[test]
    fn remove() {
        let mut ring = Ring::new(vec![1_i32, 2_i32, 3_i32, 4_i32, 5_i32, 6_i32], false);
        ring.focused = 3;
        assert_eq!(ring.focused(), Some(&4_i32));
        assert_eq!(
            ring.remove_for(&Selector::Condition(&|e| e % 2_i32 == 0_i32)),
            Some(2_i32)
        );
        // Focus follows
        assert_eq!(ring.focused(), Some(&4_i32));
    }

    #[test]
    fn indices_are_in_bounds() {
        let mut ring = Ring::new(vec![1_i32, 2_i32, 3_i32, 4_i32, 5_i32, 6_i32], false);
        assert_eq!(ring.index_for(&Selector::Index(2)), Some(2));
        assert_eq!(ring.index_for(&Selector::Index(42)), None);
    }

    #[test]
    fn map() {
        let contents = vec!["word", "filler", "another", "word"];
        let ring = Ring::new(contents.clone(), false);
        let lens = ring.map(Identify::id);
        assert_eq!(lens, vec![4, 6, 7, 4]);
        assert_eq!(ring.as_vec(), contents);
    }

    #[test]
    fn on_focused() {
        let mut ring = Ring::new(vec![1_i32, 2_i32, 3_i32], false);
        assert_eq!(ring.focused(), Some(&3_i32));

        ring.on_focused_mut(|e| *e += 1_i32);
        assert_eq!(ring.focused(), Some(&4_i32));
        assert_eq!(ring.elements, vec![1_i32, 2_i32, 4_i32]);
    }

    #[test]
    fn apply_for() {
        let contents = vec!["www", "www", "www"];
        let mut ring = Ring::new(contents.clone(), false);

        ring.apply_for_mut(&Selector::Index(2), |s| *s = "mutated");
        assert_eq!(ring.as_vec(), vec!["www", "www", "mutated"]);
    }

    #[test]
    fn on_all() {
        let mut ring = Ring::new(vec![1_i32, 2_i32, 3_i32, 4_i32], false);

        ring.on_all_for_mut(
            |e| *e += 1_i32,
            &Selector::Condition(&|e| e % 2_i32 == 0_i32),
        );
        assert_eq!(ring.elements, vec![1_i32, 3_i32, 3_i32, 5_i32]);

        ring.on_all_mut(|e| *e += 1_i32);
        assert_eq!(ring.elements, vec![2_i32, 4_i32, 4_i32, 6_i32]);
    }

    #[test]
    fn get_for() {
        let mut ring = Ring::new(vec![1_i32, 2_i32, 3_i32, 4_i32], false);
        assert_eq!(
            ring.get_for(&Selector::Condition(&|e| e % 2_i32 == 0_i32)),
            Some(&2_i32)
        );
        assert_eq!(
            ring.get_for_mut(&Selector::Condition(&|e| e % 2_i32 == 0_i32)),
            Some(&mut 2_i32)
        );

        assert_eq!(ring.get_for(&Selector::Index(2)), Some(&3_i32));
        assert_eq!(ring.get_for_mut(&Selector::Index(2)), Some(&mut 3_i32));
        assert_eq!(ring.get_for(&Selector::Index(50)), None);
        assert_eq!(ring.get_for_mut(&Selector::Index(50)), None);

        ring.focus_for(&Selector::Index(1));
        assert_eq!(ring.get_for(&Selector::Focused), Some(&2_i32));
        assert_eq!(ring.get_for_mut(&Selector::Focused), Some(&mut 2_i32));

        assert_eq!(ring.get_for(&Selector::Ident(96)), None);
        assert_eq!(ring.get_for_mut(&Selector::Ident(69)), None);

        assert_eq!(ring.as_vec(), vec![1_i32, 2_i32, 3_i32, 4_i32]);
    }

    #[test]
    fn get_all_for() {
        let mut ring = Ring::new(vec![1_i32, 2_i32, 3_i32, 4_i32], false);
        assert_eq!(
            ring.get_all_for(&Selector::Condition(&|e| e % 2_i32 == 0_i32)),
            vec![&2_i32, &4_i32]
        );
        assert_eq!(
            ring.get_all_for_mut(&Selector::Condition(&|e| e % 2_i32 == 0_i32)),
            vec![&mut 2_i32, &mut 4_i32]
        );

        assert_eq!(ring.get_all_for(&Selector::Index(2)), vec![&3_i32]);
        assert_eq!(ring.get_all_for_mut(&Selector::Index(2)), vec![&mut 3_i32]);
        assert_eq!(ring.get_all_for(&Selector::Index(50)), vec![&0_i32; 0]);
        assert_eq!(ring.get_all_for_mut(&Selector::Index(50)), vec![&0_i32; 0]);

        ring.focus_for(&Selector::Index(1));
        assert_eq!(ring.get_all_for(&Selector::Focused), vec![&2_i32]);
        assert_eq!(ring.get_all_for_mut(&Selector::Focused), vec![&mut 2_i32]);

        assert_eq!(ring.get_all_for(&Selector::Ident(96)), vec![&0_i32; 0]);
        assert_eq!(ring.get_all_for_mut(&Selector::Ident(69)), vec![&0_i32; 0]);
        assert_eq!(
            ring.get_all_for(&Selector::Condition(&|e| e % 5_i32 == 0_i32)),
            Vec::<&i32>::new()
        );

        assert_eq!(ring.as_vec(), vec![1_i32, 2_i32, 3_i32, 4_i32]);

        assert_eq!(ring.get_all_for(&Selector::First), vec![&1_i32]);
    }

    #[test]
    fn indexed_element() {
        let ring = Ring::new(vec![2_i32, 3_i32, 5_i32, 7_i32, 11_i32], false);
        assert_eq!(ring.indexed_element(&Selector::Focused), Some((4, &11_i32)));
        assert_eq!(ring.indexed_element(&Selector::Index(3)), Some((3, &7_i32)));
        assert_eq!(
            ring.indexed_element(&Selector::Condition(&|n| n % 5_i32 == 0_i32)),
            Some((2, &5_i32))
        );
    }

    #[allow(clippy::default_numeric_fallback)]
    #[test]
    fn insert_points() {
        let mut ring = Ring::new(vec![0_i32, 0_i32], false);
        ring.insert_at(&InsertPoint::Front, 1);
        assert_eq!(ring.as_vec(), vec![1, 0, 0]);
        ring.insert_at(&InsertPoint::Back, 2);
        assert_eq!(ring.as_vec(), vec![1, 0, 0, 2]);

        ring.insert_at(&InsertPoint::BeforeIndex(3), 3);
        assert_eq!(ring.as_vec(), vec![1, 0, 0, 3, 2]);
        ring.insert_at(&InsertPoint::AfterIndex(4), 6);
        assert_eq!(ring.as_vec(), vec![1, 0, 0, 3, 2, 6]);

        ring.focus_for(&Selector::Index(1));
        ring.insert_at(&InsertPoint::BeforeFocused, 4);
        assert_eq!(ring.as_vec(), vec![1, 4, 0, 0, 3, 2, 6]);
        ring.insert_at(&InsertPoint::AfterFocused, 5);
        assert_eq!(ring.as_vec(), vec![1, 4, 5, 0, 0, 3, 2, 6]);

        ring.focus_for(&Selector::Index(6));
        ring.insert_at(&InsertPoint::AfterFocused, 6);
        assert_eq!(ring.as_vec(), vec![1, 4, 5, 0, 0, 3, 2, 6, 6]);
    }
}
