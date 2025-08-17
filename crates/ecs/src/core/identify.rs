use std::{
    any::Any,
    fmt::Debug,
    hash::{BuildHasher, Hash, Hasher},
    ops::Deref,
    sync::{PoisonError, RwLock},
};

use foldhash::fast::{FixedState, FoldHasher};
use hashbrown::HashSet;

const FIXED_HASHER: FixedState =
    FixedState::with_seed(0b0101110011001100101111110101011010010101111110001100110011111110);

/// A hasher that uses a fixed seed for consistent hashing across runs.
#[derive(Copy, Clone, Default, Debug)]
pub struct StaticHasher;
impl BuildHasher for StaticHasher {
    type Hasher = FoldHasher;

    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        FIXED_HASHER.build_hasher()
    }
}

pub trait DynEq: Any {
    fn dyn_eq(&self, rhs: &dyn DynEq) -> bool;
}

impl<T: Any + Eq> DynEq for T {
    fn dyn_eq(&self, rhs: &dyn DynEq) -> bool {
        (rhs as &dyn Any)
            .downcast_ref::<T>()
            .is_some_and(|rhs| self == rhs)
    }
}

pub trait DynHash: DynEq {
    fn dyn_hash(&self, state: &mut dyn Hasher);
}

impl<T> DynHash for T
where
    T: DynEq + Hash,
{
    fn dyn_hash(&self, mut state: &mut dyn Hasher) {
        T::hash(self, &mut state);
        self.type_id().hash(&mut state);
    }
}

pub trait Identifier: Hash + Eq {
    /// Creates a static reference to `self`, possibly leaking memory.
    fn leak(&self) -> &'static Self;

    /// Returns `true` if the two references point to the same value.
    fn ref_eq(&self, other: &Self) -> bool;

    /// Feeds the reference to the hasher.
    fn ref_hash<H: Hasher>(&self, state: &mut H);
}

impl Identifier for str {
    fn leak(&self) -> &'static Self {
        let str = self.to_owned().into_boxed_str();
        Box::leak(str)
    }

    fn ref_eq(&self, other: &Self) -> bool {
        self.as_ptr() == other.as_ptr() && self.len() == other.len()
    }

    fn ref_hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        self.as_ptr().hash(state);
    }
}

pub struct Identity<T: ?Sized + 'static>(pub &'static T);

impl<T: ?Sized> Deref for Identity<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<T: ?Sized> Clone for Identity<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Identity<T> {}

/// Implements `PartialEq` for `Identity<T>` where `T` is an `Identifier`.
///
/// Two `Identity<T>` instances are considered equal if their inner values
/// are reference-equal, as determined by the `ref_eq` method on `Identifier`.
///
/// # Parameters
/// - `other`: The other `Identity<T>` to compare with.
///
/// # Returns
/// - `true` if both identities reference the same value, `false` otherwise.
impl<T: ?Sized + Identifier> PartialEq for Identity<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.ref_eq(other.0)
    }
}

impl<T: ?Sized + Identifier> Eq for Identity<T> {}

impl<T: ?Sized + Identifier> Hash for Identity<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.0.ref_hash(state);
    }
}

impl<T: ?Sized + Debug> Debug for Identity<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl<T> From<&Identity<T>> for Identity<T> {
    fn from(value: &Identity<T>) -> Self {
        *value
    }
}

pub type FixedHashSet<T> = HashSet<T, StaticHasher>;

pub struct Identifiers<T: ?Sized + 'static>(RwLock<FixedHashSet<&'static T>>);

impl<T: ?Sized + 'static> Identifiers<T> {
    pub const fn new() -> Self {
        Self(RwLock::new(FixedHashSet::<&'static T>::with_hasher(
            StaticHasher,
        )))
    }
}

impl<T: Identifier + ?Sized> Identifiers<T> {
    pub fn identify(&self, value: &T) -> Identity<T> {
        {
            let set = self.0.read().unwrap_or_else(PoisonError::into_inner);

            if let Some(value) = set.get(value) {
                return Identity(*value);
            }
        }

        {
            let mut set = self.0.write().unwrap_or_else(PoisonError::into_inner);

            if let Some(value) = set.get(value) {
                Identity(*value)
            } else {
                let leaked = value.leak();
                set.insert(leaked);
                Identity(leaked)
            }
        }
    }
}

impl<T: ?Sized> Default for Identifiers<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[macro_export]
macro_rules! define_identifier {
    (
        $(#[$label_attr:meta])*
        $trait_name:ident,
        $identifiers:ident
    ) => {

        $(#[$label_attr])*
        pub trait $trait_name: Send + Sync + ::core::fmt::Debug + $crate::core::DynEq + $crate::core::DynHash {

            /// Clones this `
            #[doc = stringify!($trait_name)]
            ///`.
            fn dyn_clone(&self) -> Box<dyn $trait_name>;

            /// Returns an [`Identity`] value corresponding to `self`.
            fn identify(&self) -> $crate::core::Identity<dyn $trait_name>
            where Self: Sized {
                $identifiers.identify(self)
            }
        }

        #[diagnostic::do_not_recommend]
        impl $trait_name for $crate::core::Identity<dyn $trait_name> {

            fn dyn_clone(&self) -> Box<dyn $trait_name> {
                (**self).dyn_clone()
            }

            fn identify(&self) -> Self {
                *self
            }
        }

        impl PartialEq for dyn $trait_name {
            fn eq(&self, other: &Self) -> bool {
                self.dyn_eq(other)
            }
        }

        impl Eq for dyn $trait_name {}

        impl ::core::hash::Hash for dyn $trait_name {
            fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                self.dyn_hash(state);
            }
        }

        impl $crate::core::Identifier for dyn $trait_name {
            fn leak(&self) -> &'static Self {
                Box::leak(self.dyn_clone())
            }

            fn ref_eq(&self, other: &Self) -> bool {
                use ::core::ptr;

                // Test that both the type id and pointer address are equivalent.
                self.type_id() == other.type_id()
                    && ptr::addr_eq(ptr::from_ref::<Self>(self), ptr::from_ref::<Self>(other))
            }

            fn ref_hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                use ::core::{hash::Hash, ptr};

                // Hash the type id...
                self.type_id().hash(state);

                // ...and the pointer address.
                // Cast to a unit `()` first to discard any pointer metadata.
                ptr::from_ref::<Self>(self).cast::<()>().hash(state);
            }
        }

        static $identifiers: $crate::core::Identifiers<dyn $trait_name> =
            $crate::core::Identifiers::new();
    };
}
