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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, PartialEq, Debug)]
    struct Health(f32);

    #[derive(Clone, PartialEq, Debug)]
    struct Name(String);

    #[test]
    fn create_entity_sequential_ids() {
        let mut w = World::new();
        assert_eq!(w.create_entity(), 0);
        assert_eq!(w.create_entity(), 1);
        assert_eq!(w.create_entity(), 2);
    }

    #[test]
    fn add_and_get_component() {
        let mut w = World::new();
        let e = w.create_entity();
        w.add_component(e, Health(100.0));
        assert_eq!(w.get_component::<Health>(e), Some(&Health(100.0)));
    }

    #[test]
    fn get_missing_component_is_none() {
        let mut w = World::new();
        let e = w.create_entity();
        assert!(w.get_component::<Health>(e).is_none());
    }

    #[test]
    fn get_component_mut_modifies_in_place() {
        let mut w = World::new();
        let e = w.create_entity();
        w.add_component(e, Health(100.0));
        w.get_component_mut::<Health>(e).unwrap().0 = 50.0;
        assert_eq!(w.get_component::<Health>(e), Some(&Health(50.0)));
    }

    #[test]
    fn has_component_true_false() {
        let mut w = World::new();
        let e = w.create_entity();
        assert!(!w.has_component::<Health>(e));
        w.add_component(e, Health(1.0));
        assert!(w.has_component::<Health>(e));
    }

    #[test]
    fn remove_component_removes_it() {
        let mut w = World::new();
        let e = w.create_entity();
        w.add_component(e, Health(1.0));
        w.remove_component::<Health>(e);
        assert!(!w.has_component::<Health>(e));
    }

    #[test]
    fn destroy_entity_removes_all_components() {
        let mut w = World::new();
        let e = w.create_entity();
        w.add_component(e, Health(1.0));
        w.add_component(e, Name("Alice".into()));
        w.destroy_entity(e);
        assert!(!w.has_component::<Health>(e));
        assert!(!w.has_component::<Name>(e));
    }

    #[test]
    fn destroy_entity_recycles_id() {
        let mut w = World::new();
        let e0 = w.create_entity();
        w.destroy_entity(e0);
        let e1 = w.create_entity(); // should reuse e0's id
        assert_eq!(e0, e1);
    }

    #[test]
    fn query_entities_returns_matching() {
        let mut w = World::new();
        let e0 = w.create_entity();
        let _e1 = w.create_entity();
        let e2 = w.create_entity();
        w.add_component(e0, Health(1.0));
        w.add_component(e2, Health(2.0));
        // e1 has no Health
        let mut found = w.query_entities::<Health>();
        found.sort();
        assert_eq!(found, vec![e0, e2]);
    }

    #[test]
    fn query_entities_empty_when_none() {
        let w = World::new();
        assert!(w.query_entities::<Health>().is_empty());
    }

    #[test]
    fn multiple_component_types_on_same_entity() {
        let mut w = World::new();
        let e = w.create_entity();
        w.add_component(e, Health(99.0));
        w.add_component(e, Name("Bob".into()));
        assert_eq!(w.get_component::<Health>(e), Some(&Health(99.0)));
        assert_eq!(w.get_component::<Name>(e), Some(&Name("Bob".into())));
    }

    #[test]
    fn iter_returns_all_pairs() {
        let mut w = World::new();
        let e0 = w.create_entity();
        let e1 = w.create_entity();
        w.add_component(e0, Health(1.0));
        w.add_component(e1, Health(2.0));
        let mut vals: Vec<f32> = w.iter::<Health>().map(|(_, h)| h.0).collect();
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(vals, vec![1.0, 2.0]);
    }

    #[test]
    fn overwrite_component() {
        let mut w = World::new();
        let e = w.create_entity();
        w.add_component(e, Health(50.0));
        w.add_component(e, Health(75.0)); // overwrite
        assert_eq!(w.get_component::<Health>(e), Some(&Health(75.0)));
    }
}
