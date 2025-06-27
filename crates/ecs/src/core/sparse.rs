use std::{
    hash::Hash,
    ops::{Index, IndexMut},
};

pub trait SparseIndex: Copy + Clone + PartialEq + Eq + Hash {
    fn to_usize(self) -> usize;
    fn from_usize(index: usize) -> Self;
}

macro_rules! impl_sparse_index {
    ($($ty:ty),+) => {
        $(impl SparseIndex for $ty {
            #[inline]
            fn to_usize(self) -> usize {
                self as usize
            }

            #[inline]
            fn from_usize(value: usize) -> Self {
                value as $ty
            }
        })*
    };
}

impl_sparse_index!(u8, u16, u32, u64, usize);

#[macro_export]
macro_rules! impl_sparse_index_wrapper {
    ($($ty:ty),+) => {
        $(impl crate::SparseIndex for $ty {
            #[inline]
            fn to_usize(self) -> usize {
                self.0 as usize
            }

            #[inline]
            fn from_usize(value: usize) -> Self {
                Self(value as u32)
            }
        })*
    };
}

pub struct SparseArray<V, I = V> {
    values: Vec<Option<V>>,
    _marker: std::marker::PhantomData<I>,
}

impl<V, I> SparseArray<V, I> {
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn push(&mut self, value: V) {
        self.values.push(Some(value));
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }
}

impl<V, I> Default for SparseArray<V, I> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V, I> SparseArray<V, I>
where
    I: SparseIndex,
{
    pub fn insert(&mut self, index: I, value: V) {
        let index = index.to_usize();
        if index >= self.values.len() {
            self.values.resize_with(index + 1, || None);
        }
        self.values[index] = Some(value);
    }

    pub fn reserve(&mut self, index: I) {
        if index.to_usize() >= self.values.len() {
            self.values.resize_with(index.to_usize() + 1, || None);
        }
    }

    pub fn get(&self, index: I) -> Option<&V> {
        let index = index.to_usize();
        if index < self.values.len() {
            self.values[index].as_ref()
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, index: I) -> Option<&mut V> {
        let index = index.to_usize();
        if index < self.values.len() {
            self.values[index].as_mut()
        } else {
            None
        }
    }

    pub fn remove(&mut self, index: I) -> Option<V> {
        let index = index.to_usize();
        self.remove_at(index)
    }

    pub fn remove_at(&mut self, index: usize) -> Option<V> {
        if index < self.values.len() {
            self.values[index].take()
        } else {
            None
        }
    }

    pub fn contains(&self, index: I) -> bool {
        let index = index.to_usize();
        if index < self.values.len() {
            self.values[index].is_some()
        } else {
            false
        }
    }
}

impl<V, I: SparseIndex> Index<I> for SparseArray<V, I> {
    type Output = Option<V>;

    fn index(&self, index: I) -> &Self::Output {
        let index = index.to_usize();
        if index < self.values.len() {
            &self.values[index]
        } else {
            &None
        }
    }
}

impl<V, I: SparseIndex> IndexMut<I> for SparseArray<V, I> {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        let index = index.to_usize();
        if index >= self.values.len() {
            self.values.resize_with(index + 1, || None);
        }
        &mut self.values[index]
    }
}

pub struct ImmutableSparseArray<V, I = V> {
    values: Box<[Option<V>]>,
    _marker: std::marker::PhantomData<I>,
}

impl<V, I> ImmutableSparseArray<V, I> {
    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl<V, I: SparseIndex> ImmutableSparseArray<V, I> {
    pub fn get(&self, index: I) -> Option<&V> {
        let index = index.to_usize();
        self.values.get(index).and_then(|v| v.as_ref())
    }

    pub fn contains(&self, index: I) -> bool {
        let index = index.to_usize();
        self.values.get(index).is_some_and(|v| v.is_some())
    }
}

impl<V, I> From<SparseArray<V, I>> for ImmutableSparseArray<V, I> {
    fn from(array: SparseArray<V, I>) -> Self {
        ImmutableSparseArray {
            values: array.values.into_boxed_slice(),
            _marker: std::marker::PhantomData,
        }
    }
}

pub struct SparseSet<V, I = V> {
    values: Vec<V>,
    indices: Vec<I>,
    sparse: SparseArray<usize, I>,
}

impl<V, I> SparseSet<V, I> {
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            indices: Vec::new(),
            sparse: SparseArray::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn values(&self) -> &[V] {
        &self.values
    }

    pub fn indices(&self) -> &[I] {
        &self.indices
    }

    pub fn iter(&self) -> impl Iterator<Item = (&I, &V)> {
        self.indices.iter().zip(self.values.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&I, &mut V)> {
        self.indices.iter().zip(self.values.iter_mut())
    }

    pub fn clear(&mut self) {
        self.values.clear();
        self.indices.clear();
        self.sparse.clear();
    }
}

impl<V, I: SparseIndex> FromIterator<(I, V)> for SparseSet<V, I> {
    fn from_iter<T: IntoIterator<Item = (I, V)>>(iter: T) -> Self {
        let mut set = SparseSet::new();
        for (index, value) in iter {
            set.insert(index, value);
        }
        set
    }
}

impl<V, I> Default for SparseSet<V, I> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V, I: SparseIndex> SparseSet<V, I> {
    pub fn get(&self, index: I) -> Option<&V> {
        let index = self.sparse.get(index)?;
        Some(&self.values[*index])
    }

    pub fn get_mut(&mut self, index: I) -> Option<&mut V> {
        let index = self.sparse.get(index)?;
        Some(&mut self.values[*index])
    }

    pub fn insert(&mut self, index: I, value: V) -> Option<V> {
        if let Some(index) = self.sparse.get(index) {
            let value = std::mem::replace(&mut self.values[*index], value);
            Some(value)
        } else {
            self.sparse.insert(index, self.values.len());
            self.values.push(value);
            self.indices.push(index);
            None
        }
    }

    pub fn remove(&mut self, index: I) -> Option<V> {
        let index = self.sparse.remove(index)?;

        self.remove_at(index).map(|(_, v)| v)
    }

    pub fn remove_at(&mut self, index: usize) -> Option<(I, V)> {
        if index >= self.values.len() {
            return None;
        }

        let value = self.values.swap_remove(index);
        let key = self.indices.swap_remove(index);
        if index != self.values.len() {
            let last_index = self.indices[index];
            self.sparse.get_mut(last_index).map(|i| *i = index);
        }
        Some((key, value))
    }

    pub fn contains(&self, index: I) -> bool {
        self.sparse.contains(index)
    }

    pub fn drain(&mut self) -> impl Iterator<Item = (I, V)> {
        self.indices
            .drain(..)
            .zip(self.values.drain(..))
            .map(|(index, value)| (index, value))
    }
}

pub type SparseSetIter<'a, V, I> = std::iter::Zip<std::slice::Iter<'a, I>, std::slice::Iter<'a, V>>;

pub struct ImmutableSparseSet<V, I = V> {
    values: Box<[V]>,
    indices: Box<[I]>,
    sparse: ImmutableSparseArray<usize, I>,
}

impl<V, I> ImmutableSparseSet<V, I> {
    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn values(&self) -> &[V] {
        &self.values
    }

    pub fn values_mut(&mut self) -> &mut [V] {
        &mut self.values
    }

    pub fn indices(&self) -> &[I] {
        &self.indices
    }

    pub fn iter(&self) -> impl Iterator<Item = (&I, &V)> {
        self.indices.iter().zip(self.values.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&I, &mut V)> {
        self.indices.iter().zip(self.values.iter_mut())
    }
}

impl<V, I: SparseIndex> ImmutableSparseSet<V, I> {
    pub fn get(&self, index: I) -> Option<&V> {
        let index = self.sparse.get(index)?;
        Some(&self.values[*index])
    }

    pub fn get_mut(&mut self, index: I) -> Option<&mut V> {
        let index = self.sparse.get(index)?;
        Some(&mut self.values[*index])
    }

    pub fn contains(&self, index: I) -> bool {
        self.sparse.contains(index)
    }
}

impl<V, I> From<SparseSet<V, I>> for ImmutableSparseSet<V, I> {
    fn from(set: SparseSet<V, I>) -> Self {
        let values = set.values.into_boxed_slice();
        let indices = set.indices.into_boxed_slice();
        let sparse = ImmutableSparseArray::from(set.sparse);

        ImmutableSparseSet {
            values,
            indices,
            sparse,
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use super::{SparseArray, SparseSet};

    #[test]
    fn sparse_array_insert() {
        let mut array = SparseArray::<u32>::new();
        array.push(10);
        array.insert(0, 20);

        let value = array[0];
        assert_eq!(value, Some(20));
    }

    #[test]
    fn sparse_array_remove() {
        let mut array = SparseArray::<u32>::new();
        array.push(10);

        let value = array.remove(0);
        assert_eq!(value, Some(10));
    }

    #[test]
    fn sparse_set_insert() {
        let mut set = SparseSet::<u32>::new();
        set.insert(0, 10);
        set.insert(1, 20);
        set.insert(10, 30);

        assert_eq!(set.get(0), Some(&10));
        assert_eq!(set.get(1), Some(&20));
        assert_eq!(set.get(10), Some(&30));
    }

    #[test]
    fn sparse_set_remove() {
        let mut set = SparseSet::<u32>::new();
        set.insert(0, 10);
        set.insert(1, 20);
        set.insert(10, 30);

        assert_eq!(set.remove(1), Some(20));
    }
}
