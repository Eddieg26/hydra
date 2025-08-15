use crate::{
    GamepadId, MouseScrollAccumulator, MouseScrollUnit,
    button::{ButtonState, Buttons},
    gamepad::{GamepadAxis, GamepadButton, Gamepads},
    keyboard::{Key, KeyCode},
    mouse::MouseButton,
};
use ecs::{
    Commands, Component, Entity, Event, Events, Query, Resource, World, WorldMode, query::With,
};
use std::{any::TypeId, collections::HashMap};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub enum InputDevice {
    #[default]
    Any,
    Keyboard,
    Gamepad(GamepadId),
}

pub struct InputMode<I: InputReceiver>(std::marker::PhantomData<I>);
impl<I: InputReceiver> WorldMode for InputMode<I> {
    const CATEGORY: &'static str = "input";
}

pub trait InputReceiver: Component {}

pub struct InputContext<'a> {
    entity: Entity,
    device: InputDevice,
    gamepads: &'a Gamepads,
    codes: &'a Buttons<KeyCode>,
    keys: &'a Buttons<Key>,
    mouse: &'a Buttons<MouseButton>,
    scrolls: &'a MouseScrollAccumulator,
}

#[derive(Debug, Clone, Copy)]
pub struct Deadzone {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
}

impl Deadzone {
    fn check(&self, axis: GamepadAxis, value: f32) -> Option<f32> {
        match axis {
            GamepadAxis::LeftStickX => (value > self.min_x && value < self.max_y).then_some(value),
            GamepadAxis::LeftStickY => (value > self.min_y && value < self.max_y).then_some(value),
            GamepadAxis::RightStickX => (value > self.min_x && value < self.max_y).then_some(value),
            GamepadAxis::RightStickY => (value > self.min_y && value < self.max_y).then_some(value),
            GamepadAxis::DPadX => (value > self.min_x && value < self.max_y).then_some(value),
            GamepadAxis::DPadY => (value > self.min_y && value < self.max_y).then_some(value),
            _ => None,
        }
    }
}

impl Default for Deadzone {
    fn default() -> Self {
        Self {
            min_x: -0.95,
            max_x: 0.95,
            min_y: -0.95,
            max_y: 0.95,
        }
    }
}

#[derive(Debug, Clone)]
pub enum InputBinding {
    Button {
        button: GamepadButton,
        state: ButtonState,
    },
    Axis {
        axis: GamepadAxis,
        deadzone: Deadzone,
    },
    DualAxis {
        x: GamepadAxis,
        y: GamepadAxis,
        deadzone: Deadzone,
    },
    KeyCode {
        code: KeyCode,
        state: ButtonState,
    },
    Key {
        key: Key,
        state: ButtonState,
    },
    Mouse {
        button: MouseButton,
        state: ButtonState,
    },
    Scroll {
        unit: MouseScrollUnit,
        deadzone: Deadzone,
    },
    Multi {
        bindings: Vec<InputBinding>,
    },
}

impl InputBinding {
    pub fn check(&self, ctx: &InputContext) -> Option<InputValue> {
        match &self {
            InputBinding::Button { button, state } => {
                if let InputDevice::Gamepad(id) = ctx.device {
                    let gamepad = ctx.gamepads.get(&id)?;
                    (gamepad.button(button).as_ref()? == state).then_some(InputValue::Bool(true))
                } else {
                    None
                }
            }
            InputBinding::Axis { axis, deadzone } => {
                if let InputDevice::Gamepad(id) = ctx.device {
                    let gamepad = ctx.gamepads.get(&id)?;
                    let value = gamepad.analog(*axis)?;
                    deadzone.check(*axis, value).map(InputValue::F32)
                } else {
                    None
                }
            }
            InputBinding::DualAxis { x, y, deadzone } => {
                if let InputDevice::Gamepad(id) = ctx.device {
                    let gamepad = ctx.gamepads.get(&id)?;
                    let x = gamepad.analog(*x).and_then(|v| deadzone.check(*x, v))?;
                    let y = gamepad.analog(*y).and_then(|v| deadzone.check(*y, v))?;
                    Some(InputValue::Vec2 { x, y })
                } else {
                    None
                }
            }
            InputBinding::KeyCode { code, state } => {
                if let InputDevice::Keyboard = ctx.device {
                    (ctx.codes.get(code) == Some(*state)).then_some(InputValue::Bool(true))
                } else {
                    None
                }
            }
            InputBinding::Key { key, state } => {
                if let InputDevice::Keyboard = ctx.device {
                    (ctx.keys.get(key) == Some(*state)).then_some(InputValue::Bool(true))
                } else {
                    None
                }
            }
            InputBinding::Mouse { button, state } => {
                (ctx.mouse.get(button) == Some(*state)).then_some(InputValue::Bool(true))
            }
            InputBinding::Scroll { unit, deadzone } => {
                let delta = ctx.scrolls.delta(unit);
                let x = deadzone.check(GamepadAxis::DPadX, delta.x as f32)?;
                let y = deadzone.check(GamepadAxis::DPadY, delta.y as f32)?;
                Some(InputValue::Vec2 { x, y })
            }
            InputBinding::Multi { bindings } => {
                let mut value = InputValue::Bool(false);
                for binding in bindings {
                    if let Some(v) = binding.check(ctx) {
                        value = value.merge(v);
                    }
                }

                if let InputValue::Bool(false) = value {
                    None
                } else {
                    Some(value)
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InputValue {
    Bool(bool),
    Int(i32),
    F32(f32),
    Vec2 { x: f32, y: f32 },
}

impl InputValue {
    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (InputValue::Bool(a), InputValue::Bool(b)) => InputValue::Bool(a || b),
            (InputValue::Int(a), InputValue::Int(b)) => InputValue::Int(a + b),
            (InputValue::F32(a), InputValue::F32(b)) => InputValue::F32(a + b),
            (InputValue::Vec2 { x: ax, y: ay }, InputValue::Vec2 { x: bx, y: by }) => {
                InputValue::Vec2 {
                    x: ax + bx,
                    y: ay + by,
                }
            }
            (InputValue::Bool(false), _) => InputValue::Bool(false),
            (InputValue::Bool(true), other) => other,
            (value, InputValue::Bool(true)) => value,
            (_, InputValue::Bool(false)) => InputValue::Bool(false),
            _ => self,
        }
    }
}

pub trait Action: Event + From<InputValue> + 'static {
    const NAME: &'static str;
}

pub struct InputAction {
    binding: InputBinding,
    send_event: fn(&mut Commands, Entity, InputValue),
}

impl InputAction {
    pub fn new<A: Action>(binding: InputBinding) -> Self {
        Self {
            binding,
            send_event: |commands, entity, value| {
                commands.add(move |world: &mut World| {
                    let events = world.resource_mut::<Events<A>>();
                    events.trigger(entity, A::from(value));
                });
            },
        }
    }

    pub fn check(&self, ctx: &InputContext) -> Option<InputValue> {
        self.binding.check(ctx)
    }

    pub(crate) fn send_event(&self, commands: &mut Commands, entity: Entity, value: InputValue) {
        (self.send_event)(commands, entity, value);
    }
}

#[derive(Default)]
pub struct Actions(HashMap<&'static str, InputAction>);

impl Actions {
    pub fn bind<A: Action>(&mut self, binding: InputBinding) {
        let action = InputAction::new::<A>(binding);
        self.0.insert(A::NAME, action);
    }

    pub fn get(&self, name: &str) -> Option<&InputAction> {
        self.0.get(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &&'static str> {
        self.0.keys()
    }

    pub fn actions(&self) -> impl Iterator<Item = &InputAction> {
        self.0.values()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&&'static str, &InputAction)> {
        self.0.iter()
    }

    pub(crate) fn execute(&self, ctx: &InputContext, commands: &mut Commands) {
        for action in self.0.values() {
            if let Some(value) = action.check(ctx) {
                action.send_event(commands, ctx.entity, value);
            }
        }
    }
}

#[derive(Resource)]
pub struct ActionRegistry(HashMap<TypeId, Actions>);

impl ActionRegistry {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn register<R: InputReceiver>(&mut self, actions: Actions) {
        self.0.insert(TypeId::of::<R>(), actions);
    }

    pub fn get<R: InputReceiver>(&self) -> Option<&Actions> {
        self.0.get(&TypeId::of::<R>())
    }

    pub(crate) fn send_action_events<R: InputReceiver>(
        mut commands: Commands,
        actions: &ActionRegistry,
        gamepads: &Gamepads,
        codes: &Buttons<KeyCode>,
        keys: &Buttons<Key>,
        mouse: &Buttons<MouseButton>,
        scrolls: &MouseScrollAccumulator,
        query: Query<(Entity, Option<&InputDevice>), With<R>>,
    ) {
        let Some(actions) = actions.get::<R>() else {
            return;
        };

        for (entity, device) in query.iter() {
            let device = device.copied().unwrap_or_default();
            let ctx = InputContext {
                entity,
                device,
                gamepads,
                codes,
                keys,
                mouse,
                scrolls,
            };

            actions.execute(&ctx, &mut commands);
        }
    }
}

// Input (W, A, S, D) -----> Action
