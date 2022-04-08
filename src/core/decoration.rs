use crate::{
    config::Config,
    geometry::{Extents, Padding},
};
use anyhow::{anyhow, Context, Result};
use std::ops::Add;

/// Borders around a window
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) struct Border {
    /// Width of the border
    pub(crate) width:  u32,
    /// Various colors for the border in a given state
    pub(crate) colors: Colorscheme,
}

impl Add<Border> for Padding {
    type Output = Self;

    fn add(self, border: Border) -> Self::Output {
        Self::Output {
            left:   self.left + border.width,
            right:  self.right + border.width,
            top:    self.top + border.width,
            bottom: self.bottom + border.width,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) struct Frame {
    pub(crate) extents: Extents,
    pub(crate) colors:  Colorscheme,
}

impl Add<Frame> for Padding {
    type Output = Self;

    fn add(self, frame: Frame) -> Self::Output {
        Self::Output {
            left:   self.left + frame.extents.left,
            right:  self.right + frame.extents.right,
            top:    self.top + frame.extents.top,
            bottom: self.bottom + frame.extents.bottom,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub(crate) struct Decoration {
    pub(crate) border: Option<Border>,
    pub(crate) frame:  Option<Frame>,
}

impl Decoration {
    pub(crate) fn extents(&self) -> Extents {
        Extents {
            left:   0,
            right:  0,
            top:    0,
            bottom: 0,
        } + *self
    }
}

impl Add<Decoration> for Padding {
    type Output = Self;

    fn add(mut self, decoration: Decoration) -> Self::Output {
        if let Some(border) = decoration.border {
            self = self + border;
        }

        if let Some(frame) = decoration.frame {
            self = self + frame;
        }

        self
    }
}

// =========================== Colorscheme ============================
// ====================================================================

pub(crate) type Color = u32;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) struct Colorscheme {
    // pub(crate) fdisowned: Color,
    // pub(crate) fsticky:   Color,
    // pub(crate) unfocused: Color,
    // pub(crate) udisowned: Color,
    // pub(crate) usticky:   Color,
    pub(crate) normal:  Color,
    pub(crate) active:  Color,
    pub(crate) focused: Color,
    pub(crate) urgent:  Color,
}

macro_rules! if_6 {
    ($c:ident) => {
        ($c.len() == 6).then(|| $c)
    };
}

impl Colorscheme {
    /// Default colorscheme (found as the default in [`Config`])
    pub(crate) const DEFAULT: Self = Self {
        normal:  0x4C_566A,
        active:  0x1E_1E1E,
        focused: 0xA9_8698,
        urgent:  0xEF_1D55,
    };

    /// Create a new [`Colorscheme`]
    pub(crate) fn new(config: &Config) -> Result<Self> {
        let to_hex = |s: &str| -> Result<u32> {
            let trim = s.strip_prefix("0x").map_or_else(
                || s.strip_prefix('#').map_or_else(|| if_6!(s), |c| if_6!(c)),
                |c| if_6!(c),
            );

            if let Some(color) = trim {
                return u32::from_str_radix(color, 16)
                    .context(format!("failed to convert {} to hex", s));
            }

            Err(anyhow!("invalid color found in configuration: {}", s))
        };

        Ok(Self {
            normal:  to_hex(&config.global.normal_border_color)?,
            active:  to_hex(&config.global.active_border_color)?,
            focused: to_hex(&config.global.focused_border_color)?,
            urgent:  to_hex(&config.global.urgent_border_color)?,
        })
    }
}

impl Default for Colorscheme {
    fn default() -> Self {
        Self::DEFAULT
    }
}
