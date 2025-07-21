use crate::{
    core::{Frame, ObjectStatus, ext, sparse::SparseIndex},
    impl_sparse_index_wrapper,
};
use std::{any::TypeId, collections::HashMap, thread::ThreadId};

pub trait Resource: Sized + 'static {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceId(pub(crate) u32);
impl_sparse_index_wrapper!(crate, ResourceId);

#[derive(Debug)]
pub struct ResourceMeta {
    name: &'static str,
    status: ObjectStatus,
    removed: Frame,
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
            status: ObjectStatus::new(),
            removed: Frame::ZERO,
            exists: false,
            send: SEND,
            offset,
            size: std::mem::size_of::<R>(),
            drop: |ptr| unsafe { std::ptr::drop_in_place(ptr as *mut R) },
            owner: if !SEND {
                Some(std::thread::current().id())
            } else {
                None
            },
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn exists(&self) -> bool {
        self.exists
    }

    pub fn added(&self) -> Frame {
        self.status.added
    }

    pub fn modified(&self) -> Frame {
        self.status.modified
    }

    pub fn removed(&self) -> Frame {
        self.removed
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

pub struct ResourceIndex<R: Resource>(usize, std::marker::PhantomData<R>);
impl<R: Resource> ResourceIndex<R> {
    pub fn get(&self) -> usize {
        self.0
    }
}

impl<R: Resource> Clone for ResourceIndex<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), Default::default())
    }
}
impl<R: Resource> Copy for ResourceIndex<R> {}

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
        let offset = ext::align_to(self.data.len(), std::mem::size_of::<usize>());
        let meta = ResourceMeta::new::<SEND, R>(offset);

        self.is_send = self.is_send && SEND;
        self.data.resize(offset + meta.size, 0);
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
            meta.status.added = frame;
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

    pub fn get_id_dynamic(&self, ty: &TypeId) -> Option<ResourceId> {
        self.index.get(ty).copied()
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

    pub fn get_mut<R: Resource>(&mut self, id: ResourceId, frame: Frame) -> Option<&mut R> {
        let id = id.to_usize();
        let meta = self.meta.get_mut(id)?;
        if !meta.exists || !meta.has_access() {
            return None;
        }

        meta.status.modified = frame;

        let data = &mut self.data[meta.offset..meta.offset + meta.size];
        Some(unsafe { &mut *(data.as_mut_ptr() as *mut R) })
    }

    pub fn get_meta(&self, id: ResourceId) -> Option<&ResourceMeta> {
        self.meta.get(id.to_usize())
    }

    pub fn remove<R: Resource>(&mut self, frame: Frame) -> Option<R> {
        let data = self.remove_by_id(self.get_id::<R>()?, frame)?;
        let resource = unsafe { std::ptr::read(data.as_ptr() as *const R) };

        return Some(resource);
    }

    pub fn remove_by_id(&mut self, id: ResourceId, frame: Frame) -> Option<Vec<u8>> {
        let id = id.to_usize();
        let meta = self.meta.get_mut(id)?;
        if !meta.exists || !meta.has_access() {
            return None;
        }
        meta.exists = false;
        meta.removed = frame;

        let data = self
            .data
            .splice(meta.offset..meta.offset + meta.size, vec![0; meta.size]);

        Some(data.collect())
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

    pub(crate) fn update(&mut self, frame: Frame) {
        for meta in &mut self.meta {
            meta.status.update(frame);
            meta.removed.update(frame);
        }
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

pub struct Cloned<R: Resource + Clone>(R);
impl<R: Resource + Clone> Cloned<R> {
    pub fn new(resource: R) -> Self {
        Self(resource)
    }
}
impl<R: Resource + Clone> std::ops::Deref for Cloned<R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<R: Resource + Clone> AsRef<R> for Cloned<R> {
    fn as_ref(&self) -> &R {
        &self.0
    }
}

impl<R: Resource + Clone> std::ops::DerefMut for Cloned<R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<R: Resource + Clone> AsMut<R> for Cloned<R> {
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
    use super::{Resource, Resources};
    use crate::core::Frame;
    use std::rc::Rc;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Value(u32);
    impl Resource for Value {}

    impl std::ops::Deref for Value {
        type Target = u32;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl std::ops::DerefMut for Value {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    #[test]
    fn resources_add() {
        let mut resources = Resources::new();
        let id = resources.add::<true, Value>(Value(10));

        assert_eq!(resources.get::<Value>(id), Some(&Value(10)));
    }

    #[test]
    fn resources_remove() {
        let mut resources = Resources::new();
        resources.add::<true, Value>(Value(10));

        let resource = resources.remove::<Value>(Frame::ZERO);

        assert_eq!(resource, Some(Value(10)));
    }

    #[test]
    fn validate_resource_access() {
        let mut resources = Resources::new();
        let id = resources.add::<false, Value>(Value(10));

        std::thread::scope(|scope| {
            scope.spawn(move || {
                let resource = resources.get::<Value>(id);
                assert!(resource.is_none());
            });
        });
    }
}
