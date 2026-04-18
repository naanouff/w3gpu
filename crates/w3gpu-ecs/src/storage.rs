use std::any::Any;
use std::collections::HashMap;

use crate::entity::Entity;

pub(crate) trait AnyStorage: Any {
    fn remove(&mut self, entity: Entity);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub(crate) struct ComponentStorage<T: 'static> {
    pub data: HashMap<Entity, T>,
}

impl<T: 'static> ComponentStorage<T> {
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }
}

impl<T: 'static> AnyStorage for ComponentStorage<T> {
    fn remove(&mut self, entity: Entity) {
        self.data.remove(&entity);
    }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
