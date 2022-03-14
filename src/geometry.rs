//! Structures used to map areas on the screen

use crate::types::{Corner, Direction, Tightness};
use std::ops::{Add, Div, Mul, Sub};
use tern::t;
// use x11rb::protocol::xproto::{self, Point as XPoint, Rectangle as XRectangle};

// =============================== Point ==============================
// ====================================================================

/// Wrapper for [`Point`](xproto::Point). When this is used with a
/// [`Rectangle`], it represents the top-left [`Corner`]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Point {
    /// X-coordinate
    x: i32,
    /// Y-coordinate
    y: i32,
}

impl Point {
    /// Create a new [`Point`]
    pub(crate) const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Return the `x` and `y` coordinates as a tuple
    pub(crate) const fn as_tuple(self) -> (i32, i32) {
        (self.x, self.y)
    }

    /// Check if [`Point`] is `(0, 0)`
    pub(crate) const fn is_zero(self) -> bool {
        self.x == 0_i32 && self.y == 0_i32
    }

    /// Check if [`Point`] is contained within the given [`Rectangle`]
    pub(crate) const fn is_inside(self, rect: Rectangle) -> bool {
        rect.is_inside(self)
    }

    /// Adds the given [`Rectangle`]'s center [`Point`] coordinates to its
    /// [`Point`] coordinates
    pub(crate) fn from_center_of_rect(rect: Rectangle) -> Self {
        let center = rect.dimension.center();

        Self {
            x: rect.point.x + center.x,
            y: rect.point.y + center.y,
        }
    }

    /// Return the [`Point`] at the center of the given [`Dimension`]
    pub(crate) fn from_center_of_dim(dim: Dimension) -> Self {
        dim.center()
    }

    /// Return the [`Point`] relative to the given [`Point`]
    pub(crate) const fn relative(self, p: Self) -> Self {
        Self {
            x: self.x - p.x,
            y: self.y - p.y,
        }
    }

    /// Return a scaled version of a [`Point`]
    pub(crate) fn scaled(self, scale: f32) -> Self {
        Self {
            x: (scale.mul(self.x as f32)) as _,
            y: (scale.mul(self.y as f32)) as _,
        }
    }
}

// ============================= Dimension ===========================
// ====================================================================

/// An a `width` and a `height`. An `area` of a [`Rectangle`]`
#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub(crate) struct Dimension {
    /// The width of the [`Rectangle`]
    width:  u32,
    /// The height of the [`Rectangle`]
    height: u32,
}

impl Default for Dimension {
    fn default() -> Self {
        Self { width: 480, height: 260 }
    }
}

impl Dimension {
    /// Create a new [`Dimension`]
    pub(crate) const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Return the `width` and `height` as a tuple
    pub(crate) const fn as_tuple(self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Check if [`Dimension`] is `(0, 0)`
    pub(crate) const fn is_zero(self) -> bool {
        self.width == 0 && self.height == 0
    }

    /// Return the center of the `width` and `height`
    pub(crate) fn center(self) -> Point {
        Point {
            x: ((self.width as f32).div(2_f32)) as i32,
            y: ((self.height as f32).div(2_f32)) as i32,
        }
    }

    /// Return the nearest [`Corner`]
    pub(crate) fn nearest_corner(self, p: Point) -> Corner {
        let center = self.center();

        match (p, center) {
            x if p.x >= center.x && p.y >= center.y => Corner::BottomRight,
            x if p.x >= center.x && p.y < center.y => Corner::TopRight,
            x if p.x < center.x && p.y >= center.y => Corner::BottomLeft,
            x if p.x < center.x && p.y > center.y => Corner::TopLeft,
            _ => unreachable!(),
        }
    }
}

impl Add<Dimension> for Point {
    type Output = Self;

    fn add(self, other: Dimension) -> Self::Output {
        Self::Output {
            x: self.x + other.width as i32,
            y: self.y + other.height as i32,
        }
    }
}

impl Sub<Dimension> for Point {
    type Output = Self;

    fn sub(self, other: Dimension) -> Self::Output {
        Self::Output {
            x: self.x - other.width as i32,
            y: self.y - other.height as i32,
        }
    }
}

impl Sub for Point {
    type Output = Dimension;

    fn sub(self, other: Self) -> Self::Output {
        Self::Output {
            width:  (self.x - other.x) as u32,
            height: (self.y - other.y) as u32,
        }
    }
}

// ============================= Rectangle ============================
// ====================================================================

/// Equivalent to `xcb_rectangle_t`
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Rectangle {
    /// Represents the top-left corner of the rectangle
    point:     Point,
    /// The width and height of the rectangle
    dimension: Dimension,
}

impl Rectangle {
    /// Create a new [`Rectangle`]
    pub(crate) const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            point:     Point::new(x, y),
            dimension: Dimension::new(width, height),
        }
    }

    /// Check if the [`Rectangle`]'s area/dimensions = 0
    pub(crate) const fn is_zero(&self) -> bool {
        self.dimension.is_zero()
    }

    /// Return the area of the [`Rectangle`]
    pub(crate) const fn area(&self) -> u32 {
        self.dimension.width * self.dimension.height
    }

    /// Check whether two [`Rectangle`]s are equivalent as a method
    pub(crate) fn equivalent(self, other: Self) -> bool {
        self == other
    }

    /// Return the top right [`Point`]
    pub(crate) const fn top_right(&self) -> Point {
        Point {
            x: self.point.x + self.dimension.width as i32,
            y: self.point.y,
        }
    }

    /// Return the bottom left [`Point`]
    pub(crate) const fn bottom_left(&self) -> Point {
        Point {
            x: self.point.x,
            y: self.point.y + self.dimension.height as i32,
        }
    }

    /// Return the bottom right [`Point`]
    pub(crate) const fn bottom_right(&self) -> Point {
        Point {
            x: self.point.x + self.dimension.width as i32,
            y: self.point.y + self.dimension.height as i32,
        }
    }

    // /// Return the closest [`Corner`]
    // pub(crate) fn nearest_corner(&self, mut p: Point) -> Corner {
    //     p += self.point.dist(Point { x: 0, y: 0 });
    //     self.dimension.nearest_corner(p)
    // }

    /// Test whether the given [`Point`] is contained within the [`Rectangle`]
    pub(crate) const fn is_inside(&self, point: Point) -> bool {
        point.x >= self.point.x
            && point.x <= self.point.x + self.dimension.width as i32
            && point.y >= self.point.y
            && point.y <= self.point.y + self.dimension.height as i32
    }

    /// Delete this function if not needed
    pub(crate) const fn contains1(&self, rect: Self) -> bool {
        self.point.x <= rect.point.x
            && (self.point.x + self.dimension.width as i32)
                >= (rect.point.x + rect.dimension.width as i32)
            && self.point.y <= rect.point.y
            && (self.point.y + self.dimension.height as i32)
                >= (rect.point.y + rect.dimension.height as i32)
    }

    // THESE ARE EQUIVALENT

    /// Test whether the given [`Rectangle`] is contained within another
    pub(crate) const fn contains(&self, rect: Self) -> bool {
        self.is_inside(rect.point) && self.is_inside(rect.bottom_right())
    }

    /// Test whether the given [`Rectangle`]'s top-left corner is contained
    /// within another [`Rectangle`], or vice-versa
    pub(crate) const fn occludes(&self, rect: Self) -> bool {
        self.is_inside(rect.point) || rect.is_inside(self.point)
    }

    /// Get the distance between the direction edge of one [`Rectangle`] and the
    /// opposite direction edge of another [`Rectangle`]
    pub(crate) const fn boundary_distance(&self, rect: Self, direction: Direction) -> u32 {
        let r1_max = Point::new(
            self.point.x + self.dimension.width as i32 - 1,
            self.point.y + self.dimension.height as i32 - 1,
        );
        let r2_max = Point::new(
            rect.point.x + rect.dimension.width as i32 - 1,
            rect.point.y + rect.dimension.height as i32 - 1,
        );

        (match direction {
            Direction::North => t!(
                r2_max.y > self.point.y
                    ? r2_max.y - self.point.y
                    : self.point.y - r2_max.y
            ),
            Direction::West => t!(
                r2_max.x > self.point.x
                    ? r2_max.x - self.point.x
                    : self.point.x - r2_max.x
            ),
            Direction::South => t!(
                rect.point.y < r1_max.y
                    ? r1_max.y - rect.point.y
                    : rect.point.y - r1_max.y
            ),
            Direction::East => t!(
                rect.point.x < r1_max.x
                    ? r1_max.x - rect.point.x
                    : rect.point.x - r1_max.x
            ),
        }) as u32
    }

    /// Is the given [`Rectangle`] on the [`Direction`] side of another
    /// [`Rectangle`]?
    pub(crate) const fn on_dir_side(
        &self,
        rect: Self,
        direction: Direction,
        tightness: Tightness,
    ) -> bool {
        let r1_max = Point::new(
            self.point.x + self.dimension.width as i32 - 1,
            self.point.y + self.dimension.height as i32 - 1,
        );
        let r2_max = Point::new(
            rect.point.x + rect.dimension.width as i32 - 1,
            rect.point.y + rect.dimension.height as i32 - 1,
        );

        match tightness {
            Tightness::Low => match direction {
                Direction::North =>
                    if rect.point.y > r1_max.y {
                        return false;
                    },
                Direction::West =>
                    if rect.point.x > r1_max.x {
                        return false;
                    },
                Direction::South =>
                    if r2_max.y < self.point.y {
                        return false;
                    },
                Direction::East =>
                    if r2_max.x < self.point.x {
                        return false;
                    },
            },
            Tightness::High => match direction {
                Direction::North =>
                    if rect.point.y >= self.point.y {
                        return false;
                    },
                Direction::West =>
                    if rect.point.x >= self.point.x {
                        return false;
                    },
                Direction::South =>
                    if r2_max.y <= r1_max.y {
                        return false;
                    },
                Direction::East =>
                    if r2_max.x <= r1_max.x {
                        return false;
                    },
            },
        }

        match direction {
            Direction::North | Direction::South => {
                return (rect.point.x >= self.point.x && rect.point.x <= r1_max.x)
                    || (r2_max.x >= self.point.x && r2_max.x <= r1_max.x)
                    || (self.point.x > rect.point.x && self.point.x < r2_max.x);
            },
            Direction::West | Direction::East => {
                return (rect.point.y >= self.point.y && rect.point.y <= r1_max.y)
                    || (r2_max.y >= self.point.y && r2_max.y <= r1_max.y)
                    || (self.point.y > rect.point.y && r1_max.y < r2_max.y);
            },
        }

        false
    }

    /// Compare two [`Rectangles`], returning custom values
    pub(crate) const fn rect_cmp(&self, rect: Self) -> i32 {
        if self.point.y >= (rect.point.y + rect.dimension.height as i32) {
            1_i32
        } else if rect.point.y >= (self.point.y + self.dimension.height as i32) {
            -1_i32
        } else if self.point.x >= (rect.point.x + rect.dimension.width as i32) {
            1_i32
        } else if rect.point.x >= (self.point.x + self.dimension.width as i32) {
            -1_i32
        } else {
            (rect.area() - self.area()) as i32
        }
    }
}