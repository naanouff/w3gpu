use std::any::TypeId;
use std::collections::{BTreeSet, HashMap};

use crate::entity::Entity;
use crate::storage::{ErasedVec, TypedVec};

/// Sorted set of TypeIds that uniquely identifies an archetype.
pub(crate) type ArchetypeKey = BTreeSet<TypeId>;

/// One archetype = one unique combination of component types.
/// Each component type is stored as a contiguous Vec<T> (column).
/// All columns have the same length = `entities.len()`.
pub(crate) struct Archetype {
    pub key: ArchetypeKey,
    pub entities: Vec<Entity>,
    /// One column per component type: TypeId → Vec<T> (type-erased).
    pub columns: HashMap<TypeId, Box<dyn ErasedVec>>,
}

impl Archetype {
    pub fn new(key: ArchetypeKey) -> Self {
        Self {
            key,
            entities: Vec::new(),
            columns: HashMap::new(),
        }
    }

    /// Add an empty typed column for T (no-op if already present).
    pub fn add_column<T: 'static>(&mut self) {
        self.columns
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(TypedVec::<T>::new()));
    }

    /// Add an empty column by cloning the column factory from another archetype.
    pub fn add_column_erased(&mut self, tid: TypeId, proto: &dyn ErasedVec) {
        self.columns.entry(tid).or_insert_with(|| proto.new_empty());
    }

    pub fn has_type(&self, tid: TypeId) -> bool {
        self.columns.contains_key(&tid)
    }

    /// Typed get — returns None if the column doesn't exist or the row is out of bounds.
    pub fn get<T: 'static>(&self, row: usize) -> Option<&T> {
        let col = self.columns.get(&TypeId::of::<T>())?;
        if row >= col.len() {
            return None;
        }
        col.get_any(row).downcast_ref()
    }

    pub fn get_mut<T: 'static>(&mut self, row: usize) -> Option<&mut T> {
        let col = self.columns.get_mut(&TypeId::of::<T>())?;
        if row >= col.len() {
            return None;
        }
        col.get_any_mut(row).downcast_mut()
    }

    /// Typed push — panics if column for T doesn't exist.
    pub fn push<T: 'static>(&mut self, val: T) {
        self.columns
            .get_mut(&TypeId::of::<T>())
            .expect("Archetype::push — no column for this type")
            .as_any_mut()
            .downcast_mut::<TypedVec<T>>()
            .unwrap()
            .data
            .push(val);
    }
}
