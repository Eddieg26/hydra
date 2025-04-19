use super::{Command, Component, Entity, World};
use crate::{BaseFilter, BaseQuery};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parent(Entity);
impl Parent {
    pub fn new(entity: Entity) -> Self {
        Self(entity)
    }

    pub fn get(&self) -> Entity {
        self.0
    }
}

impl std::ops::Deref for Parent {
    type Target = Entity;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Component for Parent {}

pub struct Children<Q = (), F = ()>(Vec<Entity>, std::marker::PhantomData<(Q, F)>);
impl Children {
    pub fn new() -> Self {
        Self(vec![], Default::default())
    }

    pub fn with_child(child: Entity) -> Self {
        Self(vec![child], Default::default())
    }

    pub fn get(&self) -> &[Entity] {
        &self.0
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Entity> {
        self.0.iter()
    }
}

impl std::ops::Deref for Children {
    type Target = [Entity];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Component for Children {}

pub struct AddChild {
    pub parent: Entity,
    pub child: Entity,
}

impl Command for AddChild {
    fn execute(self, world: &mut super::World) {
        remove_old_parent(world, self.child);

        if let Some(children) = world.get_component_mut::<Children>(self.parent) {
            children.0.push(self.child);
        } else {
            let children = Children::with_child(self.child);
            world.add_component::<Children>(self.parent, children);
        }

        world.add_component::<Parent>(self.child, Parent(self.parent));
    }
}

fn remove_old_parent(world: &mut World, child: Entity) {
    let Some(parent) = world.get_component::<Parent>(child).copied() else {
        return;
    };

    let Some(children) = world.get_component_mut::<Children>(parent.0) else {
        return;
    };

    children.0.retain(|entity| *entity != child);
    if children.0.is_empty() {
        world.remove_component::<Children>(parent.0);
    }
}

pub struct AddChildren {
    pub parent: Entity,
    pub children: Vec<Entity>,
}

// pub struct SubQuery<'w, Q: BaseQuery, F: BaseFilter = ()> {
//     query: &'w ArchetypeQuery,
//     world: WorldCell<'w>,
//     column: &'w Column,
//     state: Q::State<'w>,
//     filter: F::State<'w>,
// }

// impl<Q: BaseQuery, F: BaseFilter> BaseQuery for Children<Q, F> {
//     type Item<'w> = Vec<Q::Item<'w>>;

//     type State<'w> = Option<SubQuery<'w, Q, F>>;

//     type Data = (QueryState<Q, F>, ComponentId);

//     fn init(system: &mut SystemInit, _: &mut super::ArchetypeQuery) -> Self::Data {
//         let mut child_query = ArchetypeQuery::default();
//         let data = Q::init(system, &mut child_query);
//         let filter = F::init(system, &mut child_query);
//         let id = system.world().register::<Children>();

//         let state = QueryState {
//             query: child_query,
//             data,
//             filter,
//         };

//         (state, id)
//     }

//     fn state<'w>(
//         data: &'w Self::Data,
//         world: WorldCell<'w>,
//         archetype: &'w super::Archetype,
//         current_frame: crate::Frame,
//         system_frame: crate::Frame,
//     ) -> Self::State<'w> {
//         let column = archetype.table().get_column(data.1)?;
//         let state = Q::state(&data.0.data, world, archetype, current_frame, system_frame);
//         let filter = F::state(
//             &data.0.filter,
//             world,
//             archetype,
//             current_frame,
//             system_frame,
//         );

//         Some(SubQuery {
//             query: &data.0.query,
//             world,
//             column,
//             state,
//             filter,
//         })
//     }

//     fn get<'w>(state: &mut Self::State<'w>, _: Entity, row: super::RowIndex) -> Self::Item<'w> {
//         let Some(SubQuery {
//             query,
//             world,
//             column,
//             state,
//             filter,
//         }) = state
//         else {
//             return vec![];
//         };

//         let Some(children) = column.get::<Children>(row.to_usize()) else {
//             return vec![];
//         };

//         let world = unsafe { world.get() };

//         children
//             .0
//             .iter()
//             .filter_map(|child| {
//                 let archetype = world.archetypes().entity_archetype(*child)?;
//                 let row = archetype.table().get_entity_row(*child)?;

//                 match F::filter(filter, *child, row) && archetype.matches(&query) {
//                     true => Some(Q::get(state, *child, row)),
//                     false => None,
//                 }
//             })
//             .collect()
//     }
// }
