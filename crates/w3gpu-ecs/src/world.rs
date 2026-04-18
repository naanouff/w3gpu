use std::any::TypeId;
use std::collections::HashMap;

use crate::entity::Entity;
use crate::storage::{AnyStorage, ComponentStorage};

pub struct World {
    next_id: Entity,
    recycled: Vec<Entity>,
    stores: HashMap<TypeId, Box<dyn AnyStorage>>,
}

impl World {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            recycled: Vec::new(),
            stores: HashMap::new(),
        }
    }

    pub fn create_entity(&mut self) -> Entity {
        if let Some(id) = self.recycled.pop() {
            id
        } else {
            let id = self.next_id;
            self.next_id += 1;
            id
        }
    }

    pub fn destroy_entity(&mut self, entity: Entity) {
        for store in self.stores.values_mut() {
            store.remove(entity);
        }
        self.recycled.push(entity);
    }

    pub fn add_component<T: 'static>(&mut self, entity: Entity, component: T) {
        let store = self.stores
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(ComponentStorage::<T>::new()));
        let typed = store.as_any_mut().downcast_mut::<ComponentStorage<T>>().unwrap();
        typed.data.insert(entity, component);
    }

    pub fn get_component<T: 'static>(&self, entity: Entity) -> Option<&T> {
        let store = self.stores.get(&TypeId::of::<T>())?;
        store.as_any().downcast_ref::<ComponentStorage<T>>()?.data.get(&entity)
    }

    pub fn get_component_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        let store = self.stores.get_mut(&TypeId::of::<T>())?;
        store.as_any_mut().downcast_mut::<ComponentStorage<T>>()?.data.get_mut(&entity)
    }

    pub fn remove_component<T: 'static>(&mut self, entity: Entity) {
        if let Some(store) = self.stores.get_mut(&TypeId::of::<T>()) {
            store.as_any_mut().downcast_mut::<ComponentStorage<T>>().unwrap().data.remove(&entity);
        }
    }

    pub fn has_component<T: 'static>(&self, entity: Entity) -> bool {
        self.stores
            .get(&TypeId::of::<T>())
            .and_then(|s| s.as_any().downcast_ref::<ComponentStorage<T>>())
            .map(|s| s.data.contains_key(&entity))
            .unwrap_or(false)
    }

    /// Returns all entities that have component T.
    pub fn query_entities<T: 'static>(&self) -> Vec<Entity> {
        match self.stores.get(&TypeId::of::<T>()) {
            Some(store) => {
                let typed = store.as_any().downcast_ref::<ComponentStorage<T>>().unwrap();
                typed.data.keys().copied().collect()
            }
            None => Vec::new(),
        }
    }

    /// Iterate over all (Entity, &T) pairs for component T.
    pub fn iter<T: 'static>(&self) -> impl Iterator<Item = (Entity, &T)> {
        let store = self.stores.get(&TypeId::of::<T>());
        store
            .and_then(|s| s.as_any().downcast_ref::<ComponentStorage<T>>())
            .into_iter()
            .flat_map(|s| s.data.iter().map(|(&e, c)| (e, c)))
    }
}

impl Default for World {
    fn default() -> Self { Self::new() }
}
