use crate::{
    core::{Frame, sparse::SparseIndex},
    ext,
};
use std::{any::TypeId, collections::HashMap, thread::ThreadId};

pub trait Resource: Sized + 'static {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceId(u32);
impl SparseIndex for ResourceId {
    fn to_usize(self) -> usize {
        self.0 as usize
    }

    fn from_usize(index: usize) -> Self {
        Self(index as u32)
    }
}

pub struct ResourceMeta {
    name: &'static str,
    added: Frame,
    modified: Frame,
    exists: bool,
    send: bool,
    offset: usize,
    size: usize,
    drop: fn(*mut u8),
    owner: Option<ThreadId>,
}

impl ResourceMeta {
    pub fn new<const SEND: bool, R: Resource>(offset: usize) -> Self {
        Self {
            name: ext::short_type_name::<R>(),
            added: Frame::ZERO,
            modified: Frame::ZERO,
            exists: false,
            send: SEND,
            offset,
            size: std::mem::size_of::<R>(),
            drop: |ptr| unsafe { std::ptr::drop_in_place(ptr as *mut R) },
            owner: None,
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn exists(&self) -> bool {
        self.exists
    }

    pub fn added(&self) -> Frame {
        self.added
    }

    pub fn modified(&self) -> Frame {
        self.modified
    }

    pub fn send(&self) -> bool {
        self.send
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn owner(&self) -> Option<ThreadId> {
        self.owner
    }

    /// Check if the resource is accessible from the current thread.
    pub fn has_access(&self) -> bool {
        self.send || self.owner == Some(std::thread::current().id())
    }
}

pub struct Resources {
    data: Vec<u8>,
    meta: Vec<ResourceMeta>,
    index: HashMap<TypeId, ResourceId>,
    is_send: bool,
}

impl Resources {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            meta: Vec::new(),
            index: HashMap::new(),
            is_send: true,
        }
    }

    pub fn is_send(&self) -> bool {
        self.is_send
    }

    pub fn register<const SEND: bool, R: Resource>(&mut self) -> ResourceId {
        let id = TypeId::of::<R>();
        if let Some(id) = self.index.get(&id).copied() {
            return id;
        }

        let index = self.meta.len();
        let meta = ResourceMeta::new::<SEND, R>(self.data.len());

        self.is_send = self.is_send && SEND;
        self.data.resize(meta.offset + meta.size, 0);
        self.index.insert(id, ResourceId(index as u32));
        self.meta.push(meta);

        ResourceId(index as u32)
    }

    pub fn add<const SEND: bool, R: Resource>(&mut self, resource: R) -> ResourceId {
        self.add_with_frame::<SEND, R>(resource, Frame::ZERO)
    }

    pub fn add_with_frame<const SEND: bool, R: Resource>(
        &mut self,
        resource: R,
        frame: Frame,
    ) -> ResourceId {
        let ty = TypeId::of::<R>();
        let id = match self.index.get(&ty).copied() {
            Some(id) => id,
            None => self.register::<SEND, R>(),
        };

        let (offset, size) = {
            let meta = &mut self.meta[id.to_usize()];
            meta.added = frame;
            meta.exists = true;
            (meta.offset, meta.size)
        };

        unsafe {
            let dst = self.data[offset..offset + size].as_mut_ptr();
            std::ptr::copy_nonoverlapping(&resource as *const R as *const u8, dst, size);

            std::mem::forget(resource);
        }

        id
    }

    pub fn get_id<R: Resource>(&self) -> Option<ResourceId> {
        let id = TypeId::of::<R>();
        self.index.get(&id).copied()
    }

    pub fn get<R: Resource>(&self, id: ResourceId) -> Option<&R> {
        let id = id.to_usize();
        let meta = self.meta.get(id)?;
        if !meta.exists || !meta.has_access() {
            return None;
        }

        let data = &self.data[meta.offset..meta.offset + meta.size];
        Some(unsafe { &*(data.as_ptr() as *const R) })
    }

    pub fn get_mut<R: Resource>(&mut self, id: ResourceId) -> Option<&mut R> {
        let id = id.to_usize();
        let meta = self.meta.get(id)?;
        if !meta.exists || !meta.has_access() {
            return None;
        }

        let data = &mut self.data[meta.offset..meta.offset + meta.size];
        Some(unsafe { &mut *(data.as_mut_ptr() as *mut R) })
    }

    pub fn get_meta(&self, id: ResourceId) -> Option<&ResourceMeta> {
        self.meta.get(id.to_usize())
    }

    pub fn remove<R: Resource>(&mut self) -> Option<R> {
        let id = TypeId::of::<R>();
        let id = self.index.get(&id).copied()?;
        let meta = self.meta.get_mut(id.to_usize())?;
        if !meta.exists || !meta.has_access() {
            return None;
        }
        meta.exists = false;

        let data = &mut self.data[meta.offset..meta.offset + meta.size];
        let resource = unsafe { std::ptr::read(data.as_mut_ptr() as *const R) };

        return Some(resource);
    }

    pub fn modify(&mut self, id: ResourceId, frame: Frame) {
        let id = id.to_usize();
        if let Some(meta) = self.meta.get_mut(id) {
            if meta.exists && meta.has_access() {
                meta.modified = frame;
            }
        }
    }

    pub fn contains<R: Resource>(&self) -> bool {
        let ty = TypeId::of::<R>();
        let id = match self.index.get(&ty).copied() {
            Some(id) => id,
            None => return false,
        };

        let id = id.to_usize();
        self.meta.get(id).map_or(false, |meta| meta.exists)
    }

    pub fn len(&self) -> usize {
        self.meta.len()
    }
}

impl Drop for Resources {
    fn drop(&mut self) {
        for meta in std::mem::take(&mut self.meta) {
            if meta.exists {
                let data = &mut self.data[meta.offset..meta.offset + meta.size];
                let drop = meta.drop;
                drop(data.as_mut_ptr())
            }
        }
    }
}

pub struct NonSend<'a, R: Resource>(&'a R);
impl<'a, R: Resource> NonSend<'a, R> {
    pub fn new(resource: &'a R) -> Self {
        Self(resource)
    }
}

impl<'a, R: Resource> std::ops::Deref for NonSend<'a, R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a, R: Resource> AsRef<R> for NonSend<'a, R> {
    fn as_ref(&self) -> &R {
        self.0
    }
}

pub struct NonSendMut<'a, R: Resource>(&'a mut R);
impl<'a, R: Resource> NonSendMut<'a, R> {
    pub fn new(resource: &'a mut R) -> Self {
        Self(resource)
    }
}

impl<'a, R: Resource> std::ops::Deref for NonSendMut<'a, R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a, R: Resource> std::ops::DerefMut for NonSendMut<'a, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

impl<'a, R: Resource> AsRef<R> for NonSendMut<'a, R> {
    fn as_ref(&self) -> &R {
        self.0
    }
}

impl<'a, R: Resource> AsMut<R> for NonSendMut<'a, R> {
    fn as_mut(&mut self) -> &mut R {
        self.0
    }
}

pub struct Cloned<R: Resource>(R);
impl<R: Resource> Cloned<R> {
    pub fn new(resource: R) -> Self {
        Self(resource)
    }
}
impl<R: Resource> std::ops::Deref for Cloned<R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<R: Resource> AsRef<R> for Cloned<R> {
    fn as_ref(&self) -> &R {
        &self.0
    }
}

impl<R: Resource> std::ops::DerefMut for Cloned<R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<R: Resource> AsMut<R> for Cloned<R> {
    fn as_mut(&mut self) -> &mut R {
        &mut self.0
    }
}

impl<R: Resource + Clone> Clone for Cloned<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[allow(unused_imports)]
mod tests {
    use std::rc::Rc;

    use super::{Resource, Resources};

    impl Resource for u32 {}

    #[test]
    fn resources_add() {
        let mut resources = Resources::new();
        let id = resources.add::<true, u32>(10);

        assert_eq!(resources.get(id), Some(&10));
    }

    #[test]
    fn resources_remove() {
        let mut resources = Resources::new();
        resources.add::<true, u32>(10);

        let resource = resources.remove::<u32>();

        assert_eq!(resource, Some(10));
    }

    #[test]
    fn validate_resource_access() {
        let mut resources = Resources::new();
        let id = resources.add::<false, u32>(10);

        std::thread::scope(|scope| {
            scope.spawn(move || {
                let resource = resources.get::<u32>(id);
                assert!(resource.is_none());
            });
        });
    }
}
