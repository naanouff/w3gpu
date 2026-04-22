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
        Self {
            systems: Vec::new(),
        }
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
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systems_run_in_order() {
        let mut w = World::new();
        let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::<u32>::new()));
        let log1 = log.clone();
        let log2 = log.clone();
        let mut s = Scheduler::new();
        s.add_system(move |_: &mut World, _: f32, _: f32| log1.borrow_mut().push(1));
        s.add_system(move |_: &mut World, _: f32, _: f32| log2.borrow_mut().push(2));
        s.run(&mut w, 0.016, 0.0);
        assert_eq!(*log.borrow(), vec![1, 2]);
    }

    #[test]
    fn system_receives_delta_and_total_time() {
        let mut w = World::new();
        let received = std::rc::Rc::new(std::cell::RefCell::new((0.0f32, 0.0f32)));
        let recv = received.clone();
        let mut s = Scheduler::new();
        s.add_system(move |_: &mut World, dt: f32, t: f32| *recv.borrow_mut() = (dt, t));
        s.run(&mut w, 0.033, 1.5);
        let (dt, t) = *received.borrow();
        assert!((dt - 0.033).abs() < 1e-6);
        assert!((t - 1.5).abs() < 1e-6);
    }

    #[test]
    fn system_can_mutate_world() {
        #[derive(Clone, Debug)]
        struct Counter(u32);
        let mut w = World::new();
        let e = w.create_entity();
        w.add_component(e, Counter(0));
        let mut s = Scheduler::new();
        s.add_system(move |world: &mut World, _: f32, _: f32| {
            if let Some(c) = world.get_component_mut::<Counter>(e) {
                c.0 += 1;
            }
        });
        s.run(&mut w, 0.0, 0.0);
        s.run(&mut w, 0.0, 0.0);
        assert_eq!(w.get_component::<Counter>(e).unwrap().0, 2);
    }
}
