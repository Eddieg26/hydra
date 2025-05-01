use crate::{ComponentId, ResourceId, SparseIndex};
use fixedbitset::FixedBitSet;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AccessError {
    Component(ComponentId),
    Resource(ResourceId),
}

impl From<ComponentId> for AccessError {
    fn from(value: ComponentId) -> Self {
        Self::Component(value)
    }
}

impl From<ResourceId> for AccessError {
    fn from(value: ResourceId) -> Self {
        Self::Resource(value)
    }
}

#[derive(Clone)]
pub struct Access<I: SparseIndex> {
    read: FixedBitSet,
    write: FixedBitSet,
    _marker: std::marker::PhantomData<I>,
}

impl<I: SparseIndex> Access<I> {
    pub fn new() -> Self {
        Self {
            read: FixedBitSet::new(),
            write: FixedBitSet::new(),
            _marker: Default::default(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            read: FixedBitSet::with_capacity(capacity),
            write: FixedBitSet::with_capacity(capacity),
            _marker: Default::default(),
        }
    }

    pub fn get_read(&self) -> &FixedBitSet {
        &self.read
    }

    pub fn get_write(&self) -> &FixedBitSet {
        &self.write
    }

    pub fn reads(&self, bit: I) -> bool {
        self.read[bit.to_usize()]
    }

    pub fn writes(&self, bit: I) -> bool {
        self.write[bit.to_usize()]
    }

    pub fn read(&mut self, bit: I) -> bool {
        self.read.grow(bit.to_usize() + 1);
        self.read.set(bit.to_usize(), true);

        self.read[bit.to_usize()] && !self.write[bit.to_usize()]
    }

    pub fn write(&mut self, bit: I) -> bool {
        self.read.grow(bit.to_usize() + 1);

        let prev = self.write[bit.to_usize()];
        self.write.set(bit.to_usize(), true);

        !prev && !self.read[bit.to_usize()] && self.write[bit.to_usize()]
    }

    pub fn conflicts(&self, other: &Self) -> Result<(), usize> {
        if let Some(conflict) = self.read.intersection(&other.write).next() {
            return Err(conflict);
        };

        if let Some(conflict) = self.write.intersection(&other.write).next() {
            return Err(conflict);
        };

        if let Some(conflict) = self.write.intersection(&other.read).next() {
            return Err(conflict);
        };

        Ok(())
    }

    pub fn validate(&self) -> Result<(), usize> {
        match self.read.intersection(&self.write).next() {
            Some(conflict) => Err(conflict),
            None => Ok(()),
        }
    }

    pub fn collect(self) -> FixedBitSet {
        self.read.intersection(&self.write).collect::<FixedBitSet>()
    }
}

impl<I: SparseIndex> Into<FixedBitSet> for Access<I> {
    fn into(self) -> FixedBitSet {
        self.read.intersection(&self.write).collect::<FixedBitSet>()
    }
}

#[derive(Clone)]
pub struct ArchetypeAccess {
    pub(crate) required: Access<ComponentId>,
    pub(crate) includes: FixedBitSet,
    pub(crate) excludes: FixedBitSet,
}

impl ArchetypeAccess {
    pub fn new() -> Self {
        Self {
            required: Access::new(),
            includes: FixedBitSet::new(),
            excludes: FixedBitSet::new(),
        }
    }

    pub fn required(&self) -> &Access<ComponentId> {
        &self.required
    }

    pub fn includes(&self) -> &FixedBitSet {
        &self.includes
    }

    pub fn excludes(&self) -> &FixedBitSet {
        &self.excludes
    }

    pub fn read(&mut self, component: ComponentId) -> ComponentId {
        self.required.read(component);
        component
    }

    pub fn write(&mut self, component: ComponentId) -> ComponentId {
        self.required.write(component);
        component
    }

    pub fn include(&mut self, component: ComponentId) -> ComponentId {
        self.includes.grow(component.to_usize() + 1);
        self.includes.set(component.to_usize(), true);
        component
    }

    pub fn exclude(&mut self, component: ComponentId) -> ComponentId {
        self.excludes.grow(component.to_usize() + 1);
        self.excludes.set(component.to_usize(), true);
        component
    }

    pub fn is_disjoint(&self, other: &Self) -> bool {
        !self.required.get_read().is_disjoint(&other.excludes)
            || !self.required.get_write().is_disjoint(&other.excludes)
            || !self.includes.is_disjoint(&other.excludes)
            || !other.required.get_read().is_disjoint(&self.excludes)
            || !other.required.get_write().is_disjoint(&self.excludes)
            || !other.includes.is_disjoint(&self.excludes)
    }

    pub fn conflicts(&self, other: &Self) -> Result<(), usize> {
        if self.is_disjoint(other) {
            return Ok(());
        }

        self.required.conflicts(&other.required)
    }
}

pub struct WorldAccess {
    pub(crate) archetypes: Vec<ArchetypeAccess>,
    pub(crate) components: Access<ComponentId>,
    pub(crate) resources: Access<ResourceId>,
}

impl WorldAccess {
    pub fn new() -> Self {
        Self {
            archetypes: Vec::new(),
            components: Access::new(),
            resources: Access::new(),
        }
    }

    pub fn add_archetype(&mut self, archetype: ArchetypeAccess) {
        self.archetypes.push(archetype);
    }

    pub fn components(&self) -> &Access<ComponentId> {
        &self.components
    }

    pub fn components_mut(&mut self) -> &mut Access<ComponentId> {
        &mut self.components
    }

    pub fn resources(&self) -> &Access<ResourceId> {
        &self.resources
    }

    pub fn resources_mut(&mut self) -> &mut Access<ResourceId> {
        &mut self.resources
    }

    pub fn validate(&self) -> Result<(), AccessError> {
        if let Err(conflict) = self.resources.validate() {
            return Err(AccessError::from(ResourceId::from_usize(conflict)));
        }

        if let Err(conflict) = self.components.validate() {
            return Err(AccessError::from(ComponentId::from_usize(conflict)));
        }

        for (index, access) in self.archetypes.iter().enumerate() {
            for other in self.archetypes.iter().skip(index) {
                if let Err(conflict) = access.conflicts(other) {
                    return Err(AccessError::from(ComponentId::from_usize(conflict)));
                }
            }
        }

        Ok(())
    }

    pub fn conflicts(&self, other: &Self) -> Result<(), AccessError> {
        if let Err(conflict) = self.resources.conflicts(&other.resources) {
            return Err(AccessError::from(ResourceId::from_usize(conflict)));
        };

        if let Err(conflict) = self.components.conflicts(&other.components) {
            return Err(AccessError::from(ComponentId::from_usize(conflict)));
        };

        for access in &self.archetypes {
            if let Err(conflict) = access.required.conflicts(&other.components) {
                return Err(AccessError::from(ComponentId::from_usize(conflict)));
            }

            for other in &other.archetypes {
                if let Err(conflict) = access.conflicts(other) {
                    return Err(AccessError::from(ComponentId::from_usize(conflict)));
                }

                if let Err(conflict) = other.required.conflicts(&self.components) {
                    return Err(AccessError::from(ComponentId::from_usize(conflict)));
                }
            }
        }

        Ok(())
    }
}
