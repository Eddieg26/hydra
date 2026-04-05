use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rect<T: 'static = f32> {
    pub x: T,
    pub y: T,
    pub width: T,
    pub height: T,
}

impl<T: 'static> Rect<T> {
    pub fn new(x: T, y: T, width: T, height: T) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }
}

impl Rect {
    pub const ZERO: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    pub const ONE: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    };

    //TODO: Make generic
    pub fn intersection(&self, other: &Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let width = (self.x + self.width).min(other.x + other.width) - x;
        let height = (self.y + self.height).min(other.y + other.height) - y;
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    pub fn round(&self) -> Rect<u32> {
        Rect {
            x: self.x.round() as u32,
            y: self.y.round() as u32,
            width: self.width.round() as u32,
            height: self.height.round() as u32,
        }
    }

    pub fn ceil(&self) -> Rect<u32> {
        Rect {
            x: self.x.ceil() as u32,
            y: self.y.ceil() as u32,
            width: self.width.ceil() as u32,
            height: self.height.ceil() as u32,
        }
    }

    pub fn floor(&self) -> Rect<u32> {
        Rect {
            x: self.x.floor() as u32,
            y: self.y.floor() as u32,
            width: self.width.floor() as u32,
            height: self.height.floor() as u32,
        }
    }
}

impl<
    T: 'static
        + std::ops::Add<Output = T>
        + std::ops::Sub<Output = T>
        + std::ops::Mul<Output = T>
        + Ord
        + Copy,
> Rect<T>
{
    pub fn area(&self) -> T {
        self.width * self.height
    }

    pub fn contains_point(&self, x: T, y: T) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

impl<T: 'static + std::ops::Add<Output = T>> std::ops::Add for Rect<T> {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Rect {
            x: self.x + other.x,
            y: self.y + other.y,
            width: self.width + other.width,
            height: self.height + other.height,
        }
    }
}

impl<T: 'static + std::ops::Sub<Output = T>> std::ops::Sub for Rect<T> {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        Rect {
            x: self.x - other.x,
            y: self.y - other.y,
            width: self.width - other.width,
            height: self.height - other.height,
        }
    }
}

impl<T: 'static + std::ops::Mul<Output = T> + Copy> std::ops::Mul<T> for Rect<T> {
    type Output = Self;

    fn mul(self, scalar: T) -> Self {
        Rect {
            x: self.x * scalar,
            y: self.y * scalar,
            width: self.width * scalar,
            height: self.height * scalar,
        }
    }
}

impl<T: 'static + std::ops::Div<Output = T> + Copy> std::ops::Div<T> for Rect<T> {
    type Output = Self;

    fn div(self, scalar: T) -> Self {
        Rect {
            x: self.x / scalar,
            y: self.y / scalar,
            width: self.width / scalar,
            height: self.height / scalar,
        }
    }
}

impl<T: 'static + std::ops::AddAssign> std::ops::AddAssign for Rect<T> {
    fn add_assign(&mut self, other: Self) {
        self.x += other.x;
        self.y += other.y;
        self.width += other.width;
        self.height += other.height;
    }
}

impl<T: 'static + std::ops::SubAssign> std::ops::SubAssign for Rect<T> {
    fn sub_assign(&mut self, other: Self) {
        self.x -= other.x;
        self.y -= other.y;
        self.width -= other.width;
        self.height -= other.height;
    }
}

impl<T: 'static + std::ops::MulAssign + Copy> std::ops::MulAssign<T> for Rect<T> {
    fn mul_assign(&mut self, scalar: T) {
        self.x *= scalar;
        self.y *= scalar;
        self.width *= scalar;
        self.height *= scalar;
    }
}

impl<T: 'static + std::ops::DivAssign + Copy> std::ops::DivAssign<T> for Rect<T> {
    fn div_assign(&mut self, scalar: T) {
        self.x /= scalar;
        self.y /= scalar;
        self.width /= scalar;
        self.height /= scalar;
    }
}

impl<T: 'static + Eq> Eq for Rect<T> {}
impl<T: 'static + PartialEq> PartialEq for Rect<T> {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x
            && self.y == other.y
            && self.width == other.width
            && self.height == other.height
    }
}
