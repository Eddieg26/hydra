#[macro_export]
macro_rules! gpu_resource_id {
    ($name:ident) => {
        pub struct $name(u32);

        impl $name {
            pub fn generate() -> Self {
                static mut CURRENT: u32 = 0u32;
                unsafe {
                    let current = CURRENT;
                    CURRENT = CURRENT + 1;
                    Self(current)
                }
            }

            pub fn new(id: u32) -> Self {
                Self(id)
            }

            #[inline]
            pub fn get(&self) -> u32 {
                self.0
            }
        }

        impl std::ops::Deref for $name {
            type Target = u32;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl Clone for $name {
            fn clone(&self) -> Self {
                Self(self.0)
            }
        }

        impl Copy for $name {}

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        impl Eq for $name {}

        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                self.0.partial_cmp(&other.0)
            }
        }

        impl Ord for $name {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.0.cmp(&other.0)
            }
        }

        impl std::hash::Hash for $name {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    gpu_resource_id!(TestId);

    #[test]
    fn test_debug() {
        let id = TestId::generate();
        assert_eq!(format!("{:?}", id), "TestId(0)");
    }

    #[test]
    fn test_new() {
        let id_0 = TestId::new(0);
        let id_1 = TestId::new(1);

        assert_eq!(id_0.get(), 0);
        assert_eq!(id_1.get(), 1);
        println!("{} {}", id_0.get(), id_1.get());
    }
}
