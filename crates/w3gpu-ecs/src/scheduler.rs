use crate::world::World;

pub trait System: 'static {
    fn run(&mut self, world: &mut World, delta_time: f32, total_time: f32);
}

impl<F: FnMut(&mut World, f32, f32) + 'static> System for F {
    fn run(&mut self, world: &mut World, delta_time: f32, total_time: f32) {
        (self)(world, delta_time, total_time);
    }
}

pub struct Scheduler {
    systems: Vec<Box<dyn System>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self { systems: Vec::new() }
    }

    pub fn add_system<S: System>(&mut self, system: S) -> &mut Self {
        self.systems.push(Box::new(system));
        self
    }

    pub fn run(&mut self, world: &mut World, delta_time: f32, total_time: f32) {
        for system in &mut self.systems {
            system.run(world, delta_time, total_time);
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self { Self::new() }
}
