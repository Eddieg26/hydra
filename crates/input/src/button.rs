use ecs::Resource;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ButtonState {
    /// The element is currently pressed.
    Pressed,
    /// The element is currently released.
    Released,
}

#[derive(Resource)]
pub struct Buttons<T: Hash + Clone + Eq + 'static> {
    /// The states of the buttons.
    states: HashMap<T, ButtonState>,
    /// The buttons that are currently being held down.
    down: HashSet<T>,
}

impl<T: Hash + Clone + Eq> Default for Buttons<T> {
    fn default() -> Self {
        Self {
            states: HashMap::new(),
            down: HashSet::new(),
        }
    }
}

impl<T: Hash + Clone + Eq + std::fmt::Debug> std::fmt::Debug for Buttons<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Buttons")
            .field("states", &self.states)
            .field("down", &self.down)
            .finish()
    }
}

impl<T: Hash + Clone + Eq> Buttons<T> {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            down: HashSet::new(),
        }
    }

    pub fn set(&mut self, key: T, state: ButtonState) {
        if state == ButtonState::Pressed {
            self.down.insert(key.clone());
            self.states.insert(key, state);
        } else if let Some(ButtonState::Pressed) = self.states.get(&key) {
            self.down.remove(&key);
            self.states.insert(key, state);
        }
    }

    pub fn release_all(&mut self) {
        self.states.iter_mut().for_each(|(key, state)| {
            if *state == ButtonState::Pressed {
                *state = ButtonState::Released;
                self.down.remove(key);
            }
        });
    }

    pub fn get(&self, key: &T) -> Option<ButtonState> {
        self.states.get(key).cloned()
    }

    pub fn get_pressed(&self) -> impl Iterator<Item = &T> {
        self.states
            .iter()
            .filter_map(|(key, state)| (*state == ButtonState::Pressed).then_some(key))
    }

    pub fn get_released(&self) -> impl Iterator<Item = &T> {
        self.states
            .iter()
            .filter_map(|(key, state)| (*state == ButtonState::Released).then_some(key))
    }

    pub fn get_down(&self) -> impl Iterator<Item = &T> {
        self.down.iter()
    }

    pub fn pressed(&self, key: &T) -> bool {
        matches!(self.get(key), Some(ButtonState::Pressed))
    }

    pub fn released(&self, key: &T) -> bool {
        matches!(self.get(key), Some(ButtonState::Released))
    }

    pub fn down(&self, key: &T) -> bool {
        self.down.contains(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&T, &ButtonState)> {
        self.states.iter()
    }

    pub fn buttons(&self) -> impl Iterator<Item = &T> {
        self.states.keys()
    }

    pub fn clear(&mut self) {
        self.states.clear();
        self.down.clear();
    }
}
