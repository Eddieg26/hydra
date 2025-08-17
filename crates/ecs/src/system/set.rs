use crate::{core::Identity, define_identifier};
use std::{
    fmt::Debug,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

define_identifier!(SystemSet, SYSTEM_SET_IDENTIFIERS);

pub struct SystemType<T: 'static>(PhantomData<fn(T)>);

impl<T: 'static> SystemType<T> {
    pub(crate) fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> Debug for SystemType<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("TypeSet")
            .field(&format_args!("fn {}()", std::any::type_name::<T>()))
            .finish()
    }
}

impl<T> Hash for SystemType<T> {
    fn hash<H: Hasher>(&self, _state: &mut H) {
        // all systems of a given type are the same
    }
}

impl<T> Copy for SystemType<T> {}
impl<T> Clone for SystemType<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Eq for SystemType<T> {}
impl<T> PartialEq for SystemType<T> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        // all systems of a given type are the same
        true
    }
}

impl<T> SystemSet for SystemType<T> {
    fn dyn_clone(&self) -> Box<dyn SystemSet> {
        Box::new(*self)
    }
}

pub type SystemId = Identity<dyn SystemSet>;
