use super::World;
use crate::{Frame, SparseIndex, impl_sparse_index_wrapper};
use std::{
    any::TypeId,
    collections::HashMap,
    ops::{Index, IndexMut},
};

pub trait WorldMode: Send + Sync + 'static {
    fn enter(_: &mut World) {}
    fn exit(_: &mut World) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModeId(pub u32);
impl_sparse_index_wrapper!(ModeId);

#[derive(Clone, Copy)]
pub struct BoxedMode {
    enter: fn(&mut World),
    exit: fn(&mut World),
    frame: Frame,
}

impl BoxedMode {
    pub fn new<M: WorldMode>() -> Self {
        Self {
            enter: M::enter,
            exit: M::exit,
            frame: Frame::ZERO,
        }
    }

    pub fn frame(&self) -> Frame {
        self.frame
    }

    pub fn set_frame(&mut self, frame: Frame) {
        self.frame = frame;
    }

    pub(super) fn enter(&self, world: &mut World) {
        (self.enter)(world);
    }

    pub(super) fn exit(&self, world: &mut World) {
        (self.exit)(world);
    }
}

pub struct WorldModes {
    modes: Vec<BoxedMode>,
    map: HashMap<TypeId, ModeId>,
    pub(super) current: Option<ModeId>,
}

impl WorldModes {
    pub fn new() -> Self {
        Self {
            modes: Vec::new(),
            map: HashMap::new(),
            current: None,
        }
    }

    pub fn id<M: WorldMode>(&self) -> Option<ModeId> {
        self.map.get(&TypeId::of::<M>()).copied()
    }

    pub fn current(&self) -> Option<ModeId> {
        self.current
    }

    pub fn get(&self, id: ModeId) -> BoxedMode {
        self.modes[id.to_usize()]
    }

    pub fn add_mode<M: WorldMode>(&mut self) -> ModeId {
        let ty = TypeId::of::<M>();
        match self.map.get(&ty).copied() {
            Some(id) => id,
            None => {
                let id = ModeId::from_usize(self.modes.len());
                self.modes.push(BoxedMode::new::<M>());
                self.map.insert(ty, id);
                id
            }
        }
    }

    pub(super) fn update(&mut self, frame: Frame) {
        for mode in &mut self.modes {
            mode.frame.update(frame);
        }
    }
}

impl Index<ModeId> for WorldModes {
    type Output = BoxedMode;

    fn index(&self, index: ModeId) -> &Self::Output {
        &self.modes[index.to_usize()]
    }
}

impl IndexMut<ModeId> for WorldModes {
    fn index_mut(&mut self, index: ModeId) -> &mut Self::Output {
        &mut self.modes[index.to_usize()]
    }
}
