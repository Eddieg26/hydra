use crate::{ComponentId, ResourceId, SparseIndex};
use fixedbitset::FixedBitSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        self.write.grow(bit.to_usize() + 1);

        let prev = self.write.put(bit.to_usize());

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
        self.read.union(&self.write).collect::<FixedBitSet>()
    }
}

impl<I: SparseIndex> Into<FixedBitSet> for Access<I> {
    fn into(self) -> FixedBitSet {
        self.collect()
    }
}

#[derive(Clone)]
pub struct ArchetypeAccess {
    pub(crate) access: Access<ComponentId>,
    pub(crate) includes: FixedBitSet,
    pub(crate) excludes: FixedBitSet,
}

impl ArchetypeAccess {
    pub fn new() -> Self {
        Self {
            access: Access::new(),
            includes: FixedBitSet::new(),
            excludes: FixedBitSet::new(),
        }
    }

    pub fn access(&self) -> &Access<ComponentId> {
        &self.access
    }

    pub fn reads(&self) -> &FixedBitSet {
        &self.access.read
    }

    pub fn writes(&self) -> &FixedBitSet {
        &self.access.write
    }

    pub fn includes(&self) -> &FixedBitSet {
        &self.includes
    }

    pub fn excludes(&self) -> &FixedBitSet {
        &self.excludes
    }

    pub fn read(&mut self, component: ComponentId) -> ComponentId {
        self.access.read(component);
        self.includes.grow(component.to_usize() + 1);
        self.includes.set(component.to_usize(), true);
        component
    }

    pub fn write(&mut self, component: ComponentId) -> ComponentId {
        self.access.write(component);
        self.includes.grow(component.to_usize() + 1);
        self.includes.set(component.to_usize(), true);
        component
    }

    pub fn read_optional(&mut self, component: ComponentId) -> ComponentId {
        self.access.read(component);
        component
    }

    pub fn write_optional(&mut self, component: ComponentId) -> ComponentId {
        self.access.write(component);
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
        if self.reads().intersects(other.excludes())
            || self.writes().intersects(other.excludes())
            || self.includes().intersects(other.excludes())
            || other.reads().intersects(self.excludes())
            || other.writes().intersects(self.excludes())
            || other.includes().intersects(self.excludes())
        {
            return true;
        }

        self.reads().is_disjoint(other.writes())
            && self.writes().is_disjoint(other.writes())
            && self.writes().is_disjoint(other.reads())
    }

    pub fn conflicts(&self, other: &Self) -> Result<(), usize> {
        if self.is_disjoint(other) {
            return Ok(());
        }

        self.access.conflicts(&other.access)
    }
}

pub trait FixedBitSetExt {
    fn intersects(&self, other: &FixedBitSet) -> bool;
}

impl FixedBitSetExt for FixedBitSet {
    fn intersects(&self, other: &FixedBitSet) -> bool {
        self.intersection_count(other) > 0
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
            for other in self.archetypes.iter().skip(index + 1) {
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
            if let Err(conflict) = access.access.conflicts(&other.components) {
                return Err(AccessError::from(ComponentId::from_usize(conflict)));
            }

            for other in &other.archetypes {
                if let Err(conflict) = access.conflicts(other) {
                    return Err(AccessError::from(ComponentId::from_usize(conflict)));
                }

                if let Err(conflict) = other.access.conflicts(&self.components) {
                    return Err(AccessError::from(ComponentId::from_usize(conflict)));
                }
            }
        }

        Ok(())
    }
}

#[allow(unused_imports, dead_code)]
mod test {
    use super::WorldAccess;
    use crate::{ArchetypeAccess, ComponentId, ResourceId, SparseIndex};

    #[test]
    fn disjoint_archetype_access() {
        let mut access = ArchetypeAccess::new();
        let mut other = ArchetypeAccess::new();

        access.read(ComponentId(0));
        access.write(ComponentId(1));

        other.write(ComponentId(0));
        other.exclude(ComponentId(1));

        assert!(access.is_disjoint(&other));
    }

    #[test]
    fn world_access_conflict() {
        let mut access = WorldAccess::new();
        let mut other = WorldAccess::new();

        access.components.read(ComponentId(0));
        access.components.write(ComponentId(1));
        access.resources.read(ResourceId(0));

        other.components.read(ComponentId(0));
        other.components.write(ComponentId(2));
        other.resources.write(ResourceId(1));

        assert_eq!(access.conflicts(&other), Ok(()));
    }

    #[test]
    fn world_access_validate() {
        let mut valid = WorldAccess::new();
        valid.components.read(ComponentId(0));
        valid.components.write(ComponentId(1));
        valid.resources.read(ResourceId(0));

        assert_eq!(valid.validate(), Ok(()));

        let mut archetype = ArchetypeAccess::new();
        archetype.write(ComponentId(0));

        let mut invalid = WorldAccess::new();
        invalid.components.read(ComponentId(0));
        invalid.add_archetype(archetype);

        assert_ne!(invalid.validate(), Ok(()));
    }
}
