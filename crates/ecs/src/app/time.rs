use crate::{Plugin, Resource, Start, Update};
use derive_ecs::Phase;
use std::time::{Duration, Instant};

#[derive(Debug, Copy, Clone)]
pub struct TimeInfo {
    value: Duration,
    as_f32: f32,
    as_f64: f64,
    unscaled: Duration,
}

impl TimeInfo {
    pub fn set(&mut self, value: Duration, scale: f32) {
        self.value = value.mul_f32(scale);
        self.as_f32 = self.value.as_secs_f32();
        self.as_f64 = self.value.as_secs_f64();
        self.unscaled = value;
    }
}

impl Default for TimeInfo {
    fn default() -> Self {
        Self {
            value: Duration::ZERO,
            as_f32: 0.0,
            as_f64: 0.0,
            unscaled: Duration::ZERO,
        }
    }
}

#[derive(Resource, Debug, Copy, Clone)]
pub struct Time {
    initial: Instant,
    last_update: Option<Instant>,
    delta: TimeInfo,
    elapsed: TimeInfo,
    pub scale: f32,
}

impl Default for Time {
    fn default() -> Self {
        Self {
            initial: Instant::now(),
            last_update: None,
            delta: TimeInfo::default(),
            elapsed: TimeInfo::default(),
            scale: 1.0,
        }
    }
}

impl Time {
    pub fn new(initial: Instant) -> Self {
        Self {
            initial,
            last_update: None,
            delta: TimeInfo::default(),
            elapsed: TimeInfo::default(),
            scale: 1.0,
        }
    }

    pub fn update(&mut self, instant: Instant) {
        if let Some(last_update) = self.last_update {
            let delta = instant - last_update;
            self.increment(delta);
            self.last_update = Some(instant);
        } else {
            self.last_update = Some(instant);
        }
    }

    fn increment(&mut self, delta: Duration) {
        self.delta.set(delta, self.scale);
        self.elapsed.value += self.delta.value;
        self.elapsed.as_f32 = self.elapsed.value.as_secs_f32();
        self.elapsed.as_f64 = self.elapsed.value.as_secs_f64();
        self.elapsed.unscaled += delta;
    }

    pub fn startup(&self) -> Instant {
        self.initial
    }

    pub fn first_update(&self) -> Option<Instant> {
        self.last_update
    }

    pub fn delta(&self) -> &TimeInfo {
        &self.delta
    }

    pub fn elapsed(&self) -> &TimeInfo {
        &self.elapsed
    }
}

#[derive(Resource)]
pub struct FixedTime {
    pub timestep: Duration,
    accumulator: Duration,
}

impl Default for FixedTime {
    fn default() -> Self {
        Self {
            timestep: Self::DEFAULT_TIMESTEP,
            accumulator: Duration::ZERO,
        }
    }
}

impl FixedTime {
    /// Default timestep of 64hz (15.625ms)
    const DEFAULT_TIMESTEP: Duration = Duration::from_micros(15625);

    pub fn new(timestep: Duration) -> Self {
        Self {
            timestep,
            accumulator: Duration::default(),
        }
    }

    pub fn timestep(&self) -> Duration {
        self.timestep
    }

    pub fn accumulator(&self) -> Duration {
        self.accumulator
    }

    fn accumulate(&mut self, time: &Time) {
        self.accumulator += time.delta.value;
    }

    fn update(&mut self) -> bool {
        if let Some(accumulator) = self.accumulator.checked_sub(self.timestep) {
            self.accumulator = accumulator;
            true
        } else {
            false
        }
    }
}

#[derive(Default)]
pub struct TimePlugin;
impl Plugin for TimePlugin {
    fn setup(&mut self, app: &mut super::AppBuilder) {
        #[derive(Phase)]
        struct TimePhase;

        app.add_resource(Time::default())
            .run_before(TimePhase, Start)
            .add_systems(TimePhase, |time: &mut Time| time.update(Instant::now()));
    }
}

pub struct FixedPhase;
impl crate::Phase for FixedPhase {
    fn run(&self, ctx: crate::system::PhaseContext) {
        unsafe {
            let fixed = ctx.world().get_mut().resource_mut::<FixedTime>();
            let time = ctx.world().get().resource::<Time>();
            fixed.accumulate(time);

            if ctx.world().get_mut().resource_mut::<FixedTime>().update() {
                ctx.execute();
            }
        }
    }
}

pub struct FixedTimePlugin;
impl Plugin for FixedTimePlugin {
    fn setup(&mut self, app: &mut super::AppBuilder) {
        app.add_plugins(TimePlugin)
            .add_resource(FixedTime::default())
            .run_before(FixedPhase, Update);
    }
}
