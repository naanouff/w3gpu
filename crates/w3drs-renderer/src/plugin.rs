use w3drs_ecs::{Scheduler, World};

/// Trait pour enregistrer des fonctionnalités dans l'`App` au moment de l'init.
/// Chaque plugin reçoit un accès mutable à l'`App` pour ajouter des systèmes,
/// des ressources, ou tout autre état. Base pour Phase 3b (Archetypes) et 4 (GPU-driven).
pub trait Plugin: 'static {
    fn name(&self) -> &'static str;
    fn build(&self, app: &mut App);
}

/// Point d'entrée unifié du moteur — contient le monde ECS + le scheduler.
/// Le renderer GPU reste dans `W3drsEngine` (crate w3drs-wasm / native-triangle)
/// car il dépend de wgpu, mais les systèmes ECS s'enregistrent via `App`.
pub struct App {
    pub world: World,
    pub scheduler: Scheduler,
}

impl Default for App {
    fn default() -> Self {
        App {
            world: World::new(),
            scheduler: Scheduler::new(),
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_plugin<P: Plugin>(&mut self, plugin: P) -> &mut Self {
        plugin.build(self);
        self
    }
}
