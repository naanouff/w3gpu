use std::any::TypeId;
use std::collections::{BTreeSet, HashMap};

use crate::archetype::{Archetype, ArchetypeKey};
use crate::entity::Entity;
use crate::storage::TypedVec;

#[derive(Clone, Copy, Debug)]
struct Location {
    archetype_id: usize,
    row: usize,
}

/// ECS world backed by Archetype SoA storage.
///
/// Entities with the same set of component types share one Archetype.
/// Each component type is stored as a contiguous Vec<T>, giving cache-
/// friendly iteration while keeping O(1) per-entity lookups.
pub struct World {
    next_id: Entity,
    recycled: Vec<Entity>,
    archetypes: Vec<Archetype>,
    /// O(1) lookup: entity → (archetype index, row inside that archetype).
    entity_location: HashMap<Entity, Location>,
    /// Dedup: ArchetypeKey → archetype index.
    archetype_index: HashMap<ArchetypeKey, usize>,
}

impl World {
    pub fn new() -> Self {
        // Archetype 0 = empty archetype (all new entities land here).
        let mut archetype_index = HashMap::new();
        archetype_index.insert(BTreeSet::new(), 0usize);
        Self {
            next_id: 0,
            recycled: Vec::new(),
            archetypes: vec![Archetype::new(BTreeSet::new())],
            entity_location: HashMap::new(),
            archetype_index,
        }
    }

    // ── entity management ─────────────────────────────────────────────────

    pub fn create_entity(&mut self) -> Entity {
        let id = self.recycled.pop().unwrap_or_else(|| {
            let id = self.next_id;
            self.next_id += 1;
            id
        });
        let row = self.archetypes[0].entities.len();
        self.archetypes[0].entities.push(id);
        self.entity_location.insert(id, Location { archetype_id: 0, row });
        id
    }

    pub fn destroy_entity(&mut self, entity: Entity) {
        if let Some(loc) = self.entity_location.remove(&entity) {
            self.erase_row(loc.archetype_id, loc.row);
        }
        self.recycled.push(entity);
    }

    // ── component write ───────────────────────────────────────────────────

    pub fn add_component<T: 'static>(&mut self, entity: Entity, component: T) {
        let type_id = TypeId::of::<T>();
        let Some(loc) = self.entity_location.get(&entity).copied() else { return };

        // Fast path: archetype already has this type → replace in-place.
        if self.archetypes[loc.archetype_id].has_type(type_id) {
            self.archetypes[loc.archetype_id]
                .columns
                .get_mut(&type_id)
                .unwrap()
                .set_any(loc.row, Box::new(component));
            return;
        }

        // Build the new archetype key (old key ∪ {T}).
        let new_key: ArchetypeKey = self.archetypes[loc.archetype_id]
            .key
            .iter()
            .copied()
            .chain(std::iter::once(type_id))
            .collect();

        // Ensure the target archetype exists (with a column for T).
        if !self.archetype_index.contains_key(&new_key) {
            let mut arch = Archetype::new(new_key.clone());
            for (&tid, col) in &self.archetypes[loc.archetype_id].columns {
                arch.add_column_erased(tid, col.as_ref());
            }
            arch.add_column::<T>();
            let idx = self.archetypes.len();
            self.archetype_index.insert(new_key.clone(), idx);
            self.archetypes.push(arch);
        }
        let new_arch_id = self.archetype_index[&new_key];

        // Move entity to the new archetype, then append the new component.
        let new_row = self.migrate(entity, loc, new_arch_id);
        self.archetypes[new_arch_id].push::<T>(component);
        // Sanity: all columns must stay in sync.
        debug_assert_eq!(
            self.archetypes[new_arch_id].entities.len(),
            self.archetypes[new_arch_id].columns[&type_id].len(),
        );
        let _ = new_row; // row was already set inside migrate()
    }

    pub fn remove_component<T: 'static>(&mut self, entity: Entity) {
        let type_id = TypeId::of::<T>();
        let Some(loc) = self.entity_location.get(&entity).copied() else { return };
        if !self.archetypes[loc.archetype_id].has_type(type_id) { return; }

        // Build the new archetype key (old key ∖ {T}).
        let new_key: ArchetypeKey = self.archetypes[loc.archetype_id]
            .key
            .iter()
            .copied()
            .filter(|&tid| tid != type_id)
            .collect();

        if !self.archetype_index.contains_key(&new_key) {
            let mut arch = Archetype::new(new_key.clone());
            for (&tid, col) in &self.archetypes[loc.archetype_id].columns {
                if tid != type_id {
                    arch.add_column_erased(tid, col.as_ref());
                }
            }
            let idx = self.archetypes.len();
            self.archetype_index.insert(new_key.clone(), idx);
            self.archetypes.push(arch);
        }
        let new_arch_id = self.archetype_index[&new_key];

        // Extract just T (discard it), then migrate the rest.
        // We temporarily remove T's column from the old archetype so migrate()
        // won't try to copy it — then put it back afterwards to avoid a leak.
        let t_col = self.archetypes[loc.archetype_id].columns.remove(&type_id).unwrap();
        self.migrate(entity, loc, new_arch_id);
        // restore the column (swap_remove already shrunk it)
        self.archetypes[loc.archetype_id].columns.insert(type_id, t_col);
    }

    // ── component read ────────────────────────────────────────────────────

    pub fn get_component<T: 'static>(&self, entity: Entity) -> Option<&T> {
        let loc = self.entity_location.get(&entity)?;
        self.archetypes[loc.archetype_id].get::<T>(loc.row)
    }

    pub fn get_component_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        let loc = self.entity_location.get(&entity).copied()?;
        self.archetypes[loc.archetype_id].get_mut::<T>(loc.row)
    }

    pub fn has_component<T: 'static>(&self, entity: Entity) -> bool {
        let Some(loc) = self.entity_location.get(&entity) else { return false };
        self.archetypes[loc.archetype_id].has_type(TypeId::of::<T>())
    }

    // ── queries ───────────────────────────────────────────────────────────

    /// Returns all entities that have component T.
    pub fn query_entities<T: 'static>(&self) -> Vec<Entity> {
        let type_id = TypeId::of::<T>();
        self.archetypes
            .iter()
            .filter(|a| a.has_type(type_id))
            .flat_map(|a| a.entities.iter().copied())
            .collect()
    }

    /// Iterate (Entity, &T) over all archetypes that have T.
    /// The inner loop is over a contiguous Vec<T> — cache friendly.
    pub fn iter<T: 'static>(&self) -> impl Iterator<Item = (Entity, &T)> {
        let type_id = TypeId::of::<T>();
        self.archetypes
            .iter()
            .filter(move |a| a.has_type(type_id))
            .flat_map(move |a| {
                let col = a.columns[&type_id]
                    .as_any()
                    .downcast_ref::<TypedVec<T>>()
                    .unwrap();
                a.entities.iter().zip(col.data.iter()).map(|(&e, c)| (e, c))
            })
    }

    /// Iterate (Entity, &mut T) over all archetypes that have T.
    pub fn iter_mut<T: 'static>(&mut self) -> impl Iterator<Item = (Entity, &mut T)> {
        let type_id = TypeId::of::<T>();
        self.archetypes
            .iter_mut()
            .filter(move |a| a.has_type(type_id))
            .flat_map(move |a| {
                let entities_ptr = a.entities.as_ptr();
                let len = a.entities.len();
                let col = a.columns.get_mut(&type_id).unwrap()
                    .as_any_mut()
                    .downcast_mut::<TypedVec<T>>()
                    .unwrap();
                // SAFETY: entities and col.data are separate allocations.
                (0..len).map(move |i| {
                    let e = unsafe { *entities_ptr.add(i) };
                    let c = unsafe { &mut *(&mut col.data[i] as *mut T) };
                    (e, c)
                })
            })
    }

    /// Call `f` on every T in archetypes that do NOT contain component `Excl`.
    /// On native targets, the inner Vec<T> is processed in parallel (Rayon).
    /// On wasm32, falls back to a serial loop.
    ///
    /// Use this for systems that apply an independent per-entity update with no
    /// cross-entity dependencies (e.g. transform recomputation for flat scenes).
    pub fn for_each_without_mut<T, Excl, F>(&mut self, f: F)
    where
        T:    'static + Send + Sync,
        Excl: 'static,
        F:    Fn(&mut T) + Sync + Send,
    {
        let t_id    = TypeId::of::<T>();
        let excl_id = TypeId::of::<Excl>();

        for arch in &mut self.archetypes {
            if !arch.has_type(t_id) || arch.has_type(excl_id) { continue; }
            let Some(col) = arch.columns.get_mut(&t_id) else { continue };
            let Some(typed) = col.as_any_mut().downcast_mut::<TypedVec<T>>() else { continue };
            Self::parallel_for_each(&mut typed.data, &f);
        }
    }

    /// Call `f` on every T across all archetypes.
    /// Parallelised per-archetype on native targets.
    pub fn for_each_mut<T, F>(&mut self, f: F)
    where
        T: 'static + Send + Sync,
        F: Fn(&mut T) + Sync + Send,
    {
        let t_id = TypeId::of::<T>();
        for arch in &mut self.archetypes {
            if !arch.has_type(t_id) { continue; }
            let Some(col) = arch.columns.get_mut(&t_id) else { continue };
            let Some(typed) = col.as_any_mut().downcast_mut::<TypedVec<T>>() else { continue };
            Self::parallel_for_each(&mut typed.data, &f);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn parallel_for_each<T: Send + Sync, F: Fn(&mut T) + Sync + Send>(
        data: &mut Vec<T>, f: &F,
    ) {
        use rayon::prelude::*;
        data.par_iter_mut().for_each(f);
    }

    #[cfg(target_arch = "wasm32")]
    fn parallel_for_each<T, F: Fn(&mut T)>(data: &mut Vec<T>, f: &F) {
        data.iter_mut().for_each(f);
    }

    // ── internals ─────────────────────────────────────────────────────────

    /// Swap-remove row `row` from archetype `arch_id`, discarding all component
    /// data. Fixes up the entity that was swapped into `row`.
    fn erase_row(&mut self, arch_id: usize, row: usize) {
        let arch = &mut self.archetypes[arch_id];
        arch.entities.swap_remove(row);
        for col in arch.columns.values_mut() {
            col.swap_remove_any(row);
        }
        // Fix location of the entity that fell into `row`.
        if row < self.archetypes[arch_id].entities.len() {
            let moved = self.archetypes[arch_id].entities[row];
            self.entity_location.insert(moved, Location { archetype_id: arch_id, row });
        }
    }

    /// Move `entity` (currently at `old_loc`) to archetype `new_arch_id`.
    /// Copies all columns that exist in both archetypes.
    /// Does NOT push the newly-added component — caller handles that.
    /// Returns the new row index.
    fn migrate(&mut self, entity: Entity, old_loc: Location, new_arch_id: usize) -> usize {
        let old_arch_id = old_loc.archetype_id;
        let old_row = old_loc.row;

        // Collect old column TypeIds (avoid borrow conflict).
        let col_types: Vec<TypeId> = self.archetypes[old_arch_id]
            .columns
            .keys()
            .copied()
            .collect();

        // Step 1 — swap_remove every column and the entity list in the OLD arch.
        let mut extracted: Vec<(TypeId, Box<dyn std::any::Any>)> =
            Vec::with_capacity(col_types.len());
        {
            let old_arch = &mut self.archetypes[old_arch_id];
            old_arch.entities.swap_remove(old_row);
            for tid in &col_types {
                let val = old_arch.columns.get_mut(tid).unwrap().swap_remove_any(old_row);
                extracted.push((*tid, val));
            }
        }

        // Fix location of the entity that was swapped into old_row.
        if old_row < self.archetypes[old_arch_id].entities.len() {
            let moved = self.archetypes[old_arch_id].entities[old_row];
            self.entity_location.insert(moved, Location { archetype_id: old_arch_id, row: old_row });
        }

        // Step 2 — push extracted components into the NEW arch.
        let new_row = self.archetypes[new_arch_id].entities.len();
        self.archetypes[new_arch_id].entities.push(entity);
        for (tid, val) in extracted {
            if let Some(col) = self.archetypes[new_arch_id].columns.get_mut(&tid) {
                col.push_any(val);
            }
            // If the new archetype doesn't have this column (remove_component case),
            // the value is simply dropped here.
        }

        self.entity_location.insert(entity, Location { archetype_id: new_arch_id, row: new_row });
        new_row
    }
}

impl Default for World {
    fn default() -> Self { Self::new() }
}

// ── tests ─────────────────────────────────────────────────────────────────────

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
        let e1 = w.create_entity();
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
        w.add_component(e, Health(75.0));
        assert_eq!(w.get_component::<Health>(e), Some(&Health(75.0)));
    }

    #[test]
    fn add_component_entities_share_archetype() {
        let mut w = World::new();
        let e0 = w.create_entity();
        let e1 = w.create_entity();
        w.add_component(e0, Health(1.0));
        w.add_component(e1, Health(2.0));
        // Both entities should be in the same archetype (Health).
        let loc0 = w.entity_location[&e0];
        let loc1 = w.entity_location[&e1];
        assert_eq!(loc0.archetype_id, loc1.archetype_id);
    }

    #[test]
    fn different_component_sets_different_archetypes() {
        let mut w = World::new();
        let e0 = w.create_entity();
        let e1 = w.create_entity();
        w.add_component(e0, Health(1.0));
        w.add_component(e1, Health(1.0));
        w.add_component(e1, Name("X".into()));
        let loc0 = w.entity_location[&e0];
        let loc1 = w.entity_location[&e1];
        assert_ne!(loc0.archetype_id, loc1.archetype_id);
    }

    #[test]
    fn swap_remove_fixup_after_destroy() {
        let mut w = World::new();
        let e0 = w.create_entity();
        let e1 = w.create_entity();
        let e2 = w.create_entity();
        w.add_component(e0, Health(1.0));
        w.add_component(e1, Health(2.0));
        w.add_component(e2, Health(3.0));
        // Destroy the middle entity → e2 should be swap-moved to row 1.
        w.destroy_entity(e1);
        assert_eq!(w.get_component::<Health>(e0), Some(&Health(1.0)));
        assert_eq!(w.get_component::<Health>(e2), Some(&Health(3.0)));
        assert!(w.get_component::<Health>(e1).is_none());
    }

    #[test]
    fn iter_cache_friendly_across_archetypes() {
        let mut w = World::new();
        let e0 = w.create_entity();
        let e1 = w.create_entity();
        let e2 = w.create_entity();
        w.add_component(e0, Health(1.0));
        w.add_component(e1, Health(2.0));
        w.add_component(e1, Name("e1".into())); // different archetype from e0 and e2
        w.add_component(e2, Health(3.0));
        // iter should return all three Health values across two archetypes.
        let mut vals: Vec<f32> = w.iter::<Health>().map(|(_, h)| h.0).collect();
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(vals, vec![1.0, 2.0, 3.0]);
    }
}
