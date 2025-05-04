use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entity {
    id: u32,
    generation: u32,
}

impl Entity {
    pub fn new(id: u32, generation: u32) -> Self {
        Self { id, generation }
    }

    pub fn root(id: u32) -> Self {
        Self { id, generation: 0 }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }
}

impl std::fmt::Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Entity {{ id: {}, generation: {} }}",
            self.id, self.generation
        )
    }
}

impl Ord for Entity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl PartialOrd for Entity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.generation.partial_cmp(&other.generation) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        self.id.partial_cmp(&other.id)
    }
}

pub struct Entities {
    current: u32,
    free: Vec<u32>,
    generations: HashMap<u32, u32>,
}

impl Entities {
    pub fn new() -> Self {
        Self {
            current: 0,
            free: vec![],
            generations: HashMap::new(),
        }
    }

    pub fn spawn(&mut self) -> Entity {
        if let Some(id) = self.free.pop() {
            let generation = self.generations.entry(id).or_default();
            *generation += 1;

            Entity::new(id, *generation)
        } else {
            let id = self.current;
            let generation = 1;
            self.generations.insert(id, generation);
            self.current += 1;

            Entity::new(id, generation)
        }
    }

    pub fn despawn(&mut self, entity: Entity) {
        self.free.push(entity.id);
    }

    pub fn clear(&mut self) {
        self.current = 0;
        self.free.clear();
        self.generations.clear();
    }
}
