use serde::ser::SerializeStruct;

pub struct Size<T = f32> {
    pub width: T,
    pub height: T,
}

impl<T> Size<T> {
    pub fn new(width: T, height: T) -> Self {
        Self { width, height }
    }
}

impl<T: Copy> Copy for Size<T> {}

impl<T: Clone> Clone for Size<T> {
    fn clone(&self) -> Self {
        Size {
            width: self.width.clone(),
            height: self.height.clone(),
        }
    }
}

impl<T: Default> Default for Size<T> {
    fn default() -> Self {
        Size {
            width: T::default(),
            height: T::default(),
        }
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Size<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Size {{ width: {:?}, height: {:?} }}",
            self.width, self.height
        )
    }
}

impl<T: PartialEq> PartialEq for Size<T> {
    fn eq(&self, other: &Self) -> bool {
        self.width == other.width && self.height == other.height
    }
}

impl<T: Eq> Eq for Size<T> {}

impl<T: std::ops::Add<Output = T>> std::ops::Add for Size<T> {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Size {
            width: self.width + other.width,
            height: self.height + other.height,
        }
    }
}

impl<T: std::ops::Sub<Output = T>> std::ops::Sub for Size<T> {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        Size {
            width: self.width - other.width,
            height: self.height - other.height,
        }
    }
}

impl<T: std::ops::Mul<Output = T>> std::ops::Mul for Size<T> {
    type Output = Self;

    fn mul(self, other: Self) -> Self::Output {
        Size {
            width: self.width * other.width,
            height: self.height * other.height,
        }
    }
}

impl<T: std::ops::Div<Output = T>> std::ops::Div for Size<T> {
    type Output = Self;

    fn div(self, other: Self) -> Self::Output {
        Size {
            width: self.width / other.width,
            height: self.height / other.height,
        }
    }
}

impl<T: std::ops::Rem<Output = T>> std::ops::Rem for Size<T> {
    type Output = Self;

    fn rem(self, other: Self) -> Self::Output {
        Size {
            width: self.width % other.width,
            height: self.height % other.height,
        }
    }
}

impl<T: std::ops::Neg<Output = T>> std::ops::Neg for Size<T> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Size {
            width: -self.width,
            height: -self.height,
        }
    }
}

impl<T: std::ops::AddAssign> std::ops::AddAssign for Size<T> {
    fn add_assign(&mut self, other: Self) {
        self.width += other.width;
        self.height += other.height;
    }
}

impl<T: std::ops::SubAssign> std::ops::SubAssign for Size<T> {
    fn sub_assign(&mut self, other: Self) {
        self.width -= other.width;
        self.height -= other.height;
    }
}

impl<T: std::ops::MulAssign> std::ops::MulAssign for Size<T> {
    fn mul_assign(&mut self, other: Self) {
        self.width *= other.width;
        self.height *= other.height;
    }
}

impl<T: std::ops::DivAssign> std::ops::DivAssign for Size<T> {
    fn div_assign(&mut self, other: Self) {
        self.width /= other.width;
        self.height /= other.height;
    }
}

impl<T: std::ops::RemAssign> std::ops::RemAssign for Size<T> {
    fn rem_assign(&mut self, other: Self) {
        self.width %= other.width;
        self.height %= other.height;
    }
}

impl<T: std::ops::Add<Output = T> + Copy> std::ops::Add<T> for Size<T> {
    type Output = Self;

    fn add(self, other: T) -> Self::Output {
        Size {
            width: self.width + other,
            height: self.height + other,
        }
    }
}

impl<T: std::ops::Sub<Output = T> + Copy> std::ops::Sub<T> for Size<T> {
    type Output = Self;

    fn sub(self, other: T) -> Self::Output {
        Size {
            width: self.width - other,
            height: self.height - other,
        }
    }
}

impl<T: std::ops::Mul<Output = T> + Copy> std::ops::Mul<T> for Size<T> {
    type Output = Self;

    fn mul(self, other: T) -> Self::Output {
        Size {
            width: self.width * other,
            height: self.height * other,
        }
    }
}

impl<T: std::ops::Div<Output = T> + Copy> std::ops::Div<T> for Size<T> {
    type Output = Self;

    fn div(self, other: T) -> Self::Output {
        Size {
            width: self.width / other,
            height: self.height / other,
        }
    }
}

impl<T: std::ops::Rem<Output = T> + Copy> std::ops::Rem<T> for Size<T> {
    type Output = Self;

    fn rem(self, other: T) -> Self::Output {
        Size {
            width: self.width % other,
            height: self.height % other,
        }
    }
}

impl<T: std::ops::AddAssign + Copy> std::ops::AddAssign<T> for Size<T> {
    fn add_assign(&mut self, other: T) {
        self.width += other;
        self.height += other;
    }
}

impl<T: std::ops::SubAssign + Copy> std::ops::SubAssign<T> for Size<T> {
    fn sub_assign(&mut self, other: T) {
        self.width -= other;
        self.height -= other;
    }
}

impl<T: std::ops::MulAssign + Copy> std::ops::MulAssign<T> for Size<T> {
    fn mul_assign(&mut self, other: T) {
        self.width *= other;
        self.height *= other;
    }
}

impl<T: std::ops::DivAssign + Copy> std::ops::DivAssign<T> for Size<T> {
    fn div_assign(&mut self, other: T) {
        self.width /= other;
        self.height /= other;
    }
}

impl<T: std::ops::RemAssign + Copy> std::ops::RemAssign<T> for Size<T> {
    fn rem_assign(&mut self, other: T) {
        self.width %= other;
        self.height %= other;
    }
}

impl<T: serde::Serialize> serde::Serialize for Size<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Size", 2)?;
        state.serialize_field("width", &self.width)?;
        state.serialize_field("height", &self.height)?;
        state.end()
    }
}

impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for Size<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct SizeData<T> {
            width: T,
            height: T,
        }

        let data = SizeData::deserialize(deserializer)?;
        Ok(Size {
            width: data.width,
            height: data.height,
        })
    }
}
