use std::ops::Not;

/// Status of a switch that can be toggled on or off
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum Toggle {
    /// Status is on
    On,
    /// Status is off
    Off,
    /// Switch the current state
    Invert,
}

// FIX: Are all of these necessary?

impl From<bool> for Toggle {
    /// Convert a boolean into a [`Toggle`]
    fn from(toggle: bool) -> Self {
        if toggle {
            Self::On
        } else {
            Self::Off
        }
    }
}

impl From<Toggle> for bool {
    /// Convert a [`Toggle`] into a boolean
    fn from(t: Toggle) -> Self {
        match t {
            Toggle::On => true,
            Toggle::Off => false,
            Toggle::Invert => !Self::from(t),
        }
    }
}

impl Not for Toggle {
    type Output = Self;

    /// Invert the [`Toggle`]
    fn not(self) -> Self::Output {
        match self {
            Self::On => Self::Off,
            Self::Off => Self::On,
            Self::Invert => !self,
        }
    }
}

impl Toggle {
    /// Evaluate the [`Toggle`]
    pub(crate) const fn eval(self, current: bool) -> bool {
        match self {
            Toggle::On => true,
            Toggle::Off => false,
            Toggle::Invert => !current,
        }
    }
}
