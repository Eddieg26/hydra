#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Frame(pub u32);

impl Frame {
    pub const ZERO: Self = Self(0);

    pub fn get(self) -> u32 {
        self.0
    }

    pub fn set(&mut self, value: u32) {
        self.0 = value;
    }

    /// Checks if the current frame is newer than the target frame using the global frame as a reference.
    pub fn is_newer(self, current: Self, last: Self) -> bool {
        let last_diff = current.0.wrapping_sub(last.0);
        let target_diff = current.0.wrapping_sub(self.0);

        target_diff < last_diff
    }

    pub fn previous(self) -> Self {
        Self(self.0.wrapping_sub(1))
    }
}

impl From<u32> for Frame {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<&u32> for Frame {
    fn from(value: &u32) -> Self {
        Self(*value)
    }
}

impl std::ops::Add for Frame {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign<Self> for Frame {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl std::ops::AddAssign<u32> for Frame {
    fn add_assign(&mut self, rhs: u32) {
        self.0 += rhs;
    }
}

impl std::ops::Sub for Frame {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::SubAssign<Self> for Frame {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl std::ops::SubAssign<u32> for Frame {
    fn sub_assign(&mut self, rhs: u32) {
        self.0 -= rhs;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectStatus {
    pub added: Frame,
    pub modified: Frame,
}

impl ObjectStatus {
    pub fn new() -> Self {
        Self {
            added: Frame::ZERO,
            modified: Frame::ZERO,
        }
    }
}
