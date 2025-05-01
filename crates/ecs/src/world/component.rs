use crate::{BlobCell, SparseIndex, TypeMeta};
use fixedbitset::FixedBitSet;
use std::{alloc::Layout, any::TypeId, collections::HashMap};

pub trait Component: Send + Sync + 'static {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentId(pub(crate) u32);

pub struct ComponentMeta {
    id: ComponentId,
    meta: TypeMeta,
}

impl ComponentMeta {
    pub fn new<C: Component>(id: ComponentId) -> Self {
        Self {
            id,
            meta: TypeMeta::new::<C>(),
        }
    }

    pub fn id(&self) -> ComponentId {
        self.id
    }

    pub fn name(&self) -> &'static str {
        self.meta.name
    }

    pub fn layout(&self) -> Layout {
        self.meta.layout
    }

    pub fn drop(&self) -> Option<fn(data: *mut u8)> {
        self.meta.drop
    }
}

pub struct Components {
    components: Vec<ComponentMeta>,
    kits: Vec<FixedBitSet>,
    map: HashMap<TypeId, ComponentId>,
    kit_map: HashMap<TypeId, ComponentId>,
}

impl Components {
    pub fn new() -> Self {
        Self {
            components: vec![],
            kits: vec![],
            map: HashMap::new(),
            kit_map: HashMap::new(),
        }
    }

    pub fn register<C: Component>(&mut self) -> ComponentId {
        let ty = TypeId::of::<C>();
        match self.map.get(&ty).copied() {
            Some(id) => id,
            None => {
                let id = ComponentId::from_usize(self.components.len());
                let meta = ComponentMeta::new::<C>(id);

                self.components.push(meta);
                self.map.insert(ty, id);

                id
            }
        }
    }

    pub fn register_kit<C: ComponentKit>(&mut self) -> ComponentId {
        let ty = TypeId::of::<C>();
        match self.kit_map.get(&ty).copied() {
            Some(id) => id,
            None => {
                let id = ComponentId::from_usize(self.kits.len());
                let components = C::ids(self)
                    .iter()
                    .map(|c| c.to_usize())
                    .collect::<FixedBitSet>();

                self.kits.push(components);
                self.kit_map.insert(ty, id);

                id
            }
        }
    }

    pub fn get<C: Component>(&self) -> Option<&ComponentMeta> {
        self.map
            .get(&TypeId::of::<C>())
            .and_then(|id| self.components.get(id.to_usize()))
    }

    pub fn get_by_id(&self, id: ComponentId) -> Option<&ComponentMeta> {
        self.components.get(id.to_usize())
    }

    pub fn get_id<C: Component>(&self) -> Option<ComponentId> {
        self.get_type_id(TypeId::of::<C>())
    }

    pub fn get_type_id(&self, id: TypeId) -> Option<ComponentId> {
        self.map.get(&id).copied()
    }

    pub fn get_kit<C: ComponentKit>(&self) -> Option<&FixedBitSet> {
        self.kit_map
            .get(&TypeId::of::<C>())
            .map(|id| &self.kits[id.to_usize()])
    }

    pub unsafe fn get_id_unchecked<C: Component>(&self) -> ComponentId {
        self.map
            .get(&TypeId::of::<C>())
            .copied()
            .unwrap_or_else(|| panic!("Component not registered: {}", std::any::type_name::<C>()))
    }

    pub fn metas(&self) -> &[ComponentMeta] {
        &self.components
    }

    pub fn kits(&self) -> &[FixedBitSet] {
        &self.kits
    }

    pub fn len(&self) -> usize {
        self.components.len()
    }
}

pub struct ComponentBuffer {
    buffer: Vec<u8>,
}

impl ComponentBuffer {
    pub fn new() -> Self {
        Self { buffer: vec![] }
    }

    pub fn push<C: Component>(&mut self, component: C) -> &mut Self {
        #[repr(C, packed)]
        struct Packed<C: Component> {
            push: fn(&[u8]) -> BlobCell,
            component: C,
        }

        let packed = Packed {
            push: |bytes| -> BlobCell {
                let component = unsafe { std::ptr::read::<C>(bytes.as_ptr() as *const C) };
                BlobCell::new(component)
            },
            component,
        };

        unsafe {
            let offset = self.buffer.len();
            self.buffer.reserve(std::mem::size_of::<Packed<C>>());

            let ptr = self.buffer.as_mut_ptr().add(offset);

            ptr.cast::<Packed<C>>().write_unaligned(packed);

            self.buffer
                .set_len(offset + std::mem::size_of::<Packed<C>>());
        };

        self
    }
}

impl Iterator for ComponentBuffer {
    type Item = BlobCell;

    fn next(&mut self) -> Option<Self::Item> {
        if self.buffer.len() > 0 {
            const SIZE: usize = std::mem::size_of::<fn(&[u8]) -> BlobCell>();

            let push = unsafe {
                self.buffer[..SIZE]
                    .as_ptr()
                    .cast::<fn(&[u8]) -> BlobCell>()
                    .as_ref()
                    .unwrap_unchecked()
            };

            let cell = push(&self.buffer[SIZE..]);
            self.buffer.drain(..SIZE + cell.meta().layout.size());

            Some(cell)
        } else {
            None
        }
    }
}

pub trait ComponentKit: 'static {
    fn ids(components: &mut Components) -> Vec<ComponentId>;

    fn get(self, buffer: &mut ComponentBuffer);
}

impl<C: Component> ComponentKit for C {
    fn ids(components: &mut Components) -> Vec<ComponentId> {
        let id = components.register::<Self>();
        vec![id]
    }

    fn get(self, buffer: &mut ComponentBuffer) {
        buffer.push(self);
    }
}
