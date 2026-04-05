use ecs::IndexMap;
use std::{collections::HashMap, hash::Hash, marker::PhantomData};

pub struct BindlessId<T: 'static>(u32, PhantomData<T>);
impl<T: 'static> BindlessId<T> {
    #[inline]
    fn new(id: u32) -> Self {
        Self(id, PhantomData)
    }

    #[inline]
    pub fn get(&self) -> u32 {
        self.0
    }

    #[inline]
    pub fn usize(&self) -> usize {
        self.0 as usize
    }
}

impl<T: 'static> Copy for BindlessId<T> {}
impl<T: 'static> Clone for BindlessId<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<T: 'static> Eq for BindlessId<T> {}
impl<T: 'static> PartialEq for BindlessId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: 'static> Hash for BindlessId<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T: 'static> From<BindlessId<T>> for u32 {
    fn from(value: BindlessId<T>) -> Self {
        value.0
    }
}

impl<T: 'static> From<BindlessId<T>> for usize {
    fn from(value: BindlessId<T>) -> Self {
        value.0 as usize
    }
}

pub struct Bindless<I: Eq + Hash + Copy + Clone + 'static, R: Clone + 'static> {
    default: R,
    resources: Vec<R>,
    map: HashMap<I, BindlessId<R>>,
    queue: IndexMap<I, R>,
    free: Vec<BindlessId<R>>,
    generation: u32,
}

impl<I: Eq + Hash + Copy + Clone + 'static, R: Clone + 'static> Bindless<I, R> {
    pub fn new(default: R) -> Self {
        Self {
            default,
            resources: Vec::new(),
            map: HashMap::new(),
            queue: IndexMap::new(),
            free: Vec::new(),
            generation: 0,
        }
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    pub fn get(&self, id: &I) -> Option<BindlessId<R>> {
        self.map.get(id).copied()
    }

    pub fn queue(&mut self, id: I, resource: R) -> BindlessId<R> {
        if let Some(index) = self.map.get(&id) {
            *index
        } else {
            let len = self.resources.len();
            let index = self
                .free
                .pop()
                .unwrap_or_else(|| BindlessId::new(len as u32));

            self.resources
                .resize((index.0 + 1) as usize, self.default.clone());
            self.map.insert(id, index);
            self.queue.insert(id, resource);

            index
        }
    }

    pub fn free(&mut self, id: &I) {
        if let Some(index) = self.map.remove(id) {
            self.resources[index.usize()] = self.default.clone();
            self.queue.swap_remove(id);
            self.free.push(index);
        }
    }

    pub fn update(&mut self) {
        let Self {
            resources,
            map,
            queue,
            ..
        } = self;

        if queue.len() > 0 {
            for (id, view) in queue.drain(..) {
                let Some(index) = map.get(&id) else {
                    continue;
                };

                resources[index.usize()] = view;
            }

            self.generation += 1;
        }
    }
}
