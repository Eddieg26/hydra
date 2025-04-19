use std::{
    alloc::Layout,
    marker::PhantomData,
    ptr::{self},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeMeta {
    pub name: &'static str,
    pub layout: Layout,
    pub drop: Option<fn(data: *mut u8)>,
}

impl TypeMeta {
    pub fn new<T: 'static>() -> Self {
        Self {
            name: std::any::type_name::<T>(),
            layout: Layout::new::<T>(),
            drop: match std::mem::needs_drop::<T>() {
                true => Some(Self::drop::<T> as fn(*mut u8)),
                false => None,
            },
        }
    }

    fn drop<T>(data: *mut u8) {
        unsafe {
            let raw = data as *mut T;
            std::mem::drop(raw.read());
        }
    }
}

pub struct Blob {
    data: Vec<u8>,
    meta: TypeMeta,
}

impl Blob {
    pub fn new<T: 'static>() -> Self {
        let meta = TypeMeta::new::<T>();

        Self { data: vec![], meta }
    }

    pub unsafe fn from_raw(data: Vec<u8>, meta: TypeMeta) -> Self {
        Self { data, meta }
    }

    pub fn with_meta(meta: TypeMeta) -> Self {
        Self { data: vec![], meta }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn meta(&self) -> &TypeMeta {
        &self.meta
    }

    pub fn get<T: 'static>(&self, index: usize) -> Option<&T> {
        assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

        let offset = index * self.meta.layout.size();
        if self.data.is_empty() || offset > self.data.len() - self.meta.layout.size() {
            return None;
        }

        unsafe { (self.data.as_ptr().add(offset) as *const T).as_ref() }
    }

    pub fn get_mut<T: 'static>(&mut self, index: usize) -> Option<&mut T> {
        assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

        let offset = index * self.meta.layout.size();
        if self.data.is_empty() || offset > self.data.len() - self.meta.layout.size() {
            return None;
        }

        unsafe { (self.data.as_mut_ptr().add(offset) as *mut T).as_mut() }
    }

    pub fn push<T: 'static>(&mut self, value: T) {
        assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

        let offset = self.data.len();
        self.data
            .resize(self.data.len() + self.meta.layout.size(), 0);

        unsafe {
            let dst = self.data.as_mut_ptr().add(offset);
            ptr::write(dst as *mut T, value);
        };
    }

    pub fn insert<T: 'static>(&mut self, index: usize, value: T) {
        assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

        let offset = index * self.meta.layout.size();
        let bounds = self.data.len() - self.meta.layout.size();
        if offset > bounds {
            panic!("Index out of bounds: {}", index);
        }
        self.data
            .resize(self.data.len() + self.meta.layout.size(), 0);

        unsafe {
            let src = self.data.as_ptr().add(offset);
            let dst = self.data.as_mut_ptr().add(offset + self.meta.layout.size());

            ptr::copy(src, dst, self.data.len() - offset);
            ptr::write(src as *mut T, value);
        }
    }

    pub fn append<T: 'static>(&mut self, values: Vec<T>) {
        assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

        let offset = self.data.len();
        self.data
            .resize(offset + self.meta.layout.size() * values.len(), 0);

        unsafe {
            let src = values.as_ptr() as *mut T;
            let dst = self.data.as_mut_ptr().add(offset) as *mut T;

            ptr::copy_nonoverlapping(src, dst, values.len());

            std::mem::forget(values);
        }
    }

    pub fn remove<T: 'static>(&mut self, index: usize) -> T {
        assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

        let offset = index * self.meta.layout.size();
        if self.data.is_empty() || offset > self.data.len() - self.meta.layout.size() {
            panic!("Index out of bounds: {}", index);
        }

        unsafe {
            let src = self.data.as_ptr().add(offset) as *const T;
            let value = ptr::read::<T>(src);

            self.data.drain(offset..offset + self.meta.layout.size());

            value
        }
    }

    pub fn swap_remove<T: 'static>(&mut self, index: usize) -> T {
        assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

        let offset = index * self.meta.layout.size();
        let bounds = self.data.len() - self.meta.layout.size();

        if offset > bounds {
            panic!("Index out of bounds: {}", index);
        }

        unsafe {
            let dst = self.data.as_mut_ptr().add(offset) as *mut T;
            let src = self.data.as_ptr().add(bounds) as *const T;

            let value = ptr::read(dst);
            if offset != bounds {
                ptr::copy_nonoverlapping(src, dst, 1);
            }

            self.data.set_len(bounds);

            value
        }
    }

    pub unsafe fn append_raw(&mut self, value: Vec<u8>) {
        assert!(value.len() % self.meta.layout.size() == 0);

        self.data.extend(value);
    }

    pub unsafe fn insert_raw(&mut self, index: usize, value: Vec<u8>) {
        let offset = index * self.meta.layout.size();
        if self.data.is_empty() || offset > self.data.len() - self.meta.layout.size() {
            panic!("Index out of bounds: {}", index);
        }
        self.data.resize(self.data.len() + value.len(), 0);

        unsafe {
            let src = self.data.as_ptr().add(offset);
            let dst = self.data.as_mut_ptr().add(offset + self.meta.layout.size());

            ptr::copy(src, dst, self.data.len() - offset);
            ptr::copy_nonoverlapping(value.as_ptr(), src as *mut u8, value.len());
        }
    }

    pub unsafe fn remove_raw(&mut self, index: usize) -> Vec<u8> {
        let offset = index * self.meta.layout.size();
        if self.data.is_empty() || offset > self.data.len() - self.meta.layout.size() {
            panic!("Index out of bounds: {}", index);
        }

        self.data
            .drain(offset..offset + self.meta.layout.size())
            .collect()
    }

    pub unsafe fn swap_remove_raw(&mut self, index: usize) -> Vec<u8> {
        let offset = index * self.meta.layout.size();
        if self.data.is_empty() || offset > self.data.len() - self.meta.layout.size() {
            panic!("Index out of bounds: {}", index);
        }

        unsafe {
            let mut bytes = vec![0u8; self.meta.layout.size()];
            let src = self
                .data
                .as_ptr()
                .add(self.data.len() - self.meta.layout.size());
            ptr::copy_nonoverlapping(src, bytes.as_mut_ptr(), bytes.len());

            let bytes = self
                .data
                .splice(offset..offset + self.meta.layout.size(), bytes)
                .collect::<Vec<_>>();

            self.data.set_len(self.data.len() - self.meta.layout.size());

            bytes
        }
    }

    pub unsafe fn ptr<T: 'static>(&self) -> Ptr<'_, T> {
        unsafe { Ptr::new(self.data.as_ptr() as *mut T) }
    }

    pub fn len(&self) -> usize {
        self.data.len() / self.meta.layout.size()
    }

    pub fn is_empty(&self) -> bool {
        self.data.len() == 0
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn into_raw(mut self) -> (Vec<u8>, TypeMeta) {
        (std::mem::take(&mut self.data), self.meta)
    }

    pub fn to_vec<T: 'static>(self) -> Vec<T> {
        unsafe {
            let values = Vec::from_raw_parts(
                self.data.as_ptr() as *mut T,
                self.len(),
                self.data.capacity() / self.meta.layout.size(),
            );

            std::mem::forget(self);

            values
        }
    }
}

impl Drop for Blob {
    fn drop(&mut self) {
        if let Some(drop) = self.meta.drop {
            for index in 0..self.len() {
                let offset = index * self.meta.layout.size();
                let value = unsafe { self.data.as_mut_ptr().add(offset) };
                drop(value);
            }
        }

        self.data.clear();
    }
}

impl From<BlobCell> for Blob {
    fn from(value: BlobCell) -> Self {
        let blob = Self {
            data: unsafe {
                Vec::from_raw_parts(
                    value.data.as_ptr() as *mut u8,
                    value.data.len(),
                    value.data.capacity(),
                )
            },
            meta: value.meta,
        };

        std::mem::forget(value);

        blob
    }
}

pub struct BlobCell {
    data: Vec<u8>,
    meta: TypeMeta,
}

impl BlobCell {
    pub fn new<T: 'static>(value: T) -> Self {
        let meta = TypeMeta::new::<T>();
        let mut data = vec![0u8; meta.layout.size()];

        unsafe { ptr::write(data.as_mut_ptr() as *mut T, value) };

        Self { data, meta }
    }

    pub unsafe fn from_raw(data: Vec<u8>, meta: TypeMeta) -> Self {
        Self { data, meta }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn meta(&self) -> &TypeMeta {
        &self.meta
    }

    pub fn get<T: 'static>(&self) -> &T {
        assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

        unsafe { (self.data.as_ptr() as *const T).as_ref().unwrap() }
    }

    pub fn get_mut<T: 'static>(&mut self) -> &mut T {
        assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

        unsafe { (self.data.as_mut_ptr() as *mut T).as_mut().unwrap() }
    }

    pub fn into_raw(mut self) -> (Vec<u8>, TypeMeta) {
        let data = std::mem::take(&mut self.data);
        let meta = self.meta;

        std::mem::forget(self);

        (data, meta)
    }

    pub fn into_value<T: 'static>(self) -> T {
        assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

        let value = unsafe { std::ptr::read(self.data.as_ptr() as *const T) };

        std::mem::forget(self);

        value
    }
}

impl Drop for BlobCell {
    fn drop(&mut self) {
        if let Some(drop) = self.meta.drop {
            let value = self.data.as_mut_ptr();
            drop(value);
        }

        self.data.clear();
    }
}

pub struct Ptr<'a, T: 'static> {
    data: *mut T,
    _marker: PhantomData<&'a T>,
}

impl<'a, T: 'static> Ptr<'a, T> {
    pub unsafe fn new(data: *mut T) -> Self {
        Self {
            data,
            _marker: Default::default(),
        }
    }

    pub unsafe fn get(&self, index: usize) -> Option<&'a T> {
        if index < std::mem::size_of::<T>() {
            Some(unsafe { &*self.data.add(index) })
        } else {
            None
        }
    }

    pub unsafe fn get_mut(&mut self, index: usize) -> Option<&'a mut T> {
        if index < std::mem::size_of::<T>() {
            Some(unsafe { &mut *self.data.add(index) })
        } else {
            None
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use super::{Blob, BlobCell, TypeMeta};

    #[test]
    fn blob_from_raw() {
        let values = [10, 20, 30, 40];
        let mut bytes = vec![0u8; std::mem::size_of::<i32>() * 4];
        unsafe {
            std::ptr::copy_nonoverlapping(
                values.as_ptr() as *const u8,
                bytes.as_mut_ptr(),
                std::mem::size_of::<i32>() * 4,
            )
        };

        let meta = TypeMeta::new::<i32>();

        let blob = unsafe { Blob::from_raw(bytes, meta) };
        for (index, value) in values.iter().enumerate() {
            assert_eq!(blob.get::<i32>(index), Some(value));
        }
    }

    #[test]
    fn blob_push_and_get() {
        let mut blob = Blob::new::<i32>();
        blob.push(10);
        blob.push(20);
        blob.push(30);

        assert_eq!(blob.get(0), Some(&10));
        assert_eq!(blob.get(1), Some(&20));
        assert_eq!(blob.get(2), Some(&30));
    }

    #[test]
    fn blob_insert_and_get_mut() {
        let mut blob = Blob::new::<i32>();
        blob.push(10);
        blob.push(30);
        blob.push(40);
        blob.insert(1, 20);

        assert_eq!(blob.get(0), Some(&10));
        assert_eq!(blob.get(1), Some(&20));
        assert_eq!(blob.get(2), Some(&30));
        assert_eq!(blob.get_mut(3), Some(&mut 40));
    }

    #[test]
    fn blob_append() {
        let values = vec![10, 20, 30, 40];
        let mut blob = Blob::new::<i32>();
        blob.append(values.clone());

        for (index, value) in values.iter().enumerate() {
            assert_eq!(blob.get::<i32>(index), Some(value));
        }
    }

    #[test]
    fn blob_remove() {
        let mut blob = Blob::new::<i32>();
        blob.push(10);
        blob.push(20);

        let value = blob.remove::<i32>(1);
        assert_eq!(value, 20);

        let value = blob.remove::<i32>(0);
        assert_eq!(value, 10);
    }

    #[test]
    fn blob_swap_remove() {
        let mut blob = Blob::new::<i32>();
        blob.push(10);
        blob.push(20);
        blob.push(30);

        let value = blob.swap_remove::<i32>(0);
        assert_eq!(value, 10);

        let value = blob.get::<i32>(0);
        assert_eq!(value, Some(&30));
    }

    #[test]
    fn blob_append_raw() {
        let values = [10, 20, 30, 40];
        let mut bytes = vec![0u8; std::mem::size_of::<i32>() * 4];
        unsafe {
            std::ptr::copy_nonoverlapping(
                values.as_ptr() as *const u8,
                bytes.as_mut_ptr(),
                std::mem::size_of::<i32>() * 4,
            )
        };

        let mut blob = Blob::new::<i32>();
        unsafe { blob.append_raw(bytes) };

        for (index, value) in values.iter().enumerate() {
            assert_eq!(blob.get::<i32>(index), Some(value));
        }
    }

    #[test]
    fn blob_insert_raw() {
        let value = 20;
        let mut bytes = vec![0u8; std::mem::size_of::<i32>()];
        unsafe {
            std::ptr::copy_nonoverlapping(
                std::ptr::addr_of!(value) as *const u8,
                bytes.as_mut_ptr(),
                std::mem::size_of::<i32>(),
            )
        };

        let mut blob = Blob::new::<i32>();
        blob.push(10);
        blob.push(30);
        blob.push(40);

        unsafe { blob.insert_raw(1, bytes) };
        assert_eq!(blob.get(0), Some(&10));
        assert_eq!(blob.get(1), Some(&20));
        assert_eq!(blob.get(2), Some(&30));
        assert_eq!(blob.get(3), Some(&40));
    }

    #[test]
    fn blob_remove_raw() {
        let mut blob = Blob::new::<i32>();
        blob.push(10);

        let bytes = unsafe { blob.remove_raw(0) };
        let value = unsafe { (bytes.as_ptr() as *const i32).as_ref().unwrap() };
        assert_eq!(value, &10);
    }

    #[test]
    fn blob_swap_remove_raw() {
        let mut blob = Blob::new::<i32>();
        blob.push(10);
        blob.push(20);
        blob.push(30);

        let bytes = unsafe { blob.swap_remove_raw(0) };
        let value = unsafe { (bytes.as_ptr() as *const i32).as_ref().unwrap() };
        assert_eq!(value, &10);
        assert_eq!(blob.get(0), Some(&30));
    }

    #[test]
    fn blob_to_vec() {
        let values = vec![10, 20, 30, 40];
        let mut blob = Blob::new::<i32>();
        blob.append(values.clone());

        assert_eq!(values, blob.to_vec::<i32>());
    }

    #[test]
    fn blob_from_blob_cell() {
        let cell = BlobCell::new(10);
        let blob = Blob::from(cell);

        assert_eq!(blob.get(0), Some(&10));
    }

    #[test]
    fn blob_cell_into_blob() {
        let cell = BlobCell::new(10);
        let blob: Blob = cell.into();

        assert_eq!(blob.get(0), Some(&10));
    }

    #[test]
    fn blob_cell_new() {
        let blob = BlobCell::new(10);

        assert_eq!(blob.get::<i32>(), &10);
    }

    #[test]
    fn blob_cell_from_raw() {
        let value = 10;
        let mut bytes = vec![0u8; std::mem::size_of::<i32>()];
        unsafe {
            std::ptr::write(bytes.as_mut_ptr() as *mut i32, value);
        }

        let meta = TypeMeta::new::<i32>();
        let blob = unsafe { BlobCell::from_raw(bytes, meta) };

        assert_eq!(blob.get::<i32>(), &10);
    }

    #[test]
    fn blob_cell_into_value() {
        let blob = BlobCell::new(10);

        assert_eq!(blob.into_value::<i32>(), 10);
    }
}
