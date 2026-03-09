use ecs::ext::short_type_name;
use std::{
    fmt::{Debug, Display},
    hash::Hash,
    marker::PhantomData,
};

pub struct GpuResourceId<T: 'static>(u32, PhantomData<T>);

impl<T: 'static> GpuResourceId<T> {
    pub(crate) fn generate() -> Self {
        static mut CURRENT: u32 = 0u32;
        unsafe {
            let current = CURRENT;
            CURRENT = CURRENT + 1;
            Self(current, PhantomData)
        }
    }

    pub(crate) fn new(id: u32) -> Self {
        Self(id, PhantomData)
    }

    #[inline]
    pub fn get(&self) -> u32 {
        self.0
    }
}

impl<T: 'static> std::ops::Deref for GpuResourceId<T> {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: 'static> Clone for GpuResourceId<T> {
    fn clone(&self) -> Self {
        Self(self.0, self.1.clone())
    }
}

impl<T: 'static> Copy for GpuResourceId<T> {}

impl<T: 'static> PartialEq for GpuResourceId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl<T: 'static> Eq for GpuResourceId<T> {}

impl<T: 'static> PartialOrd for GpuResourceId<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<T: 'static> Ord for GpuResourceId<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T: 'static> Hash for GpuResourceId<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T: 'static> Debug for GpuResourceId<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", short_type_name::<T>(), self.0)
    }
}

impl<T: 'static> Display for GpuResourceId<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", short_type_name::<T>(), self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::GpuResourceId;
    use wgpu::Buffer;

    #[test]
    fn test_debug() {
        let id = GpuResourceId::<Buffer>::generate();

        assert_eq!(format!("{:?}", id), "Buffer(0)");
    }

    #[test]
    fn test_new() {
        let id_0 = GpuResourceId::<Buffer>::generate();
        let id_1 = GpuResourceId::<Buffer>::generate();

        assert!(id_0.get() == 0);
        assert!(id_1.get() == 1);
    }
}
